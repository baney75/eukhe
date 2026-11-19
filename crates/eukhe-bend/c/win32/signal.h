// The UCRT's <signal.h> plus what core.c needs of POSIX signals on Windows.
//
// sigaction(SIGSEGV) records the handler that c/win32/sys/mman.h's
// exception handler runs on an access violation the commit-on-touch does
// not resolve, a PROT_NONE guard page above all (pool_stack and err_trap
// in core.c): a guard hit still reaches the trap. Windows reports no bus
// error, so a SIGBUS handler is recorded and never runs. sigaltstack is
// accepted and changes nothing: the exception handler runs on the faulting
// thread's own stack, which a guard hit of the runtime's stack (a heap
// mapping) leaves whole.
//
// Windows has no SIGPIPE: a write to a closed pipe fails with EPIPE, as
// under SIG_IGN, so signal(SIGPIPE, ...) changes nothing, and
// raise(SIGPIPE) returns: core.c's io_lost then ends the process with its
// own fail-stop. The UCRT's signal and raise end the process on a signal
// they do not know, so these two go through eb_w32_signal and eb_w32_raise.
#ifndef EUKHE_BEND_SIGNAL_H
#define EUKHE_BEND_SIGNAL_H

#include_next <signal.h>

#include <errno.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define SIGBUS     10
#define SIGPIPE    13
#define SA_ONSTACK 0x08000000
#define SA_NODEFER 0x40000000
#define SS_DISABLE 4
#define SIGSTKSZ   8192

typedef struct {
  void*  ss_sp;
  int    ss_flags;
  size_t ss_size;
} stack_t;

struct sigaction {
  void (*sa_handler)(int);
  unsigned sa_mask;
  int      sa_flags;
};

// The handlers sigaction recorded for SIGSEGV and SIGBUS; 0: none (the
// default action, a crash).
static _Atomic uintptr_t eb_w32_segv_handler;
static _Atomic uintptr_t eb_w32_bus_handler;

static inline _Atomic uintptr_t* eb_w32_handler_slot(int sig) {
  return sig == SIGSEGV ? &eb_w32_segv_handler : sig == SIGBUS ? &eb_w32_bus_handler : NULL;
}

// The handler the fault path runs for sig, or NULL.
static inline void (*eb_w32_trap_handler(int sig))(int) {
  _Atomic uintptr_t* slot = eb_w32_handler_slot(sig);
  return slot == NULL ? NULL : (void (*)(int))atomic_load(slot);
}

static inline int sigaction(int sig, const struct sigaction* act, struct sigaction* old) {
  _Atomic uintptr_t* slot = eb_w32_handler_slot(sig);
  if (slot == NULL) {
    errno = EINVAL;
    return -1;
  }
  if (old != NULL) {
    void (*was)(int) = eb_w32_trap_handler(sig);
    *old = (struct sigaction){ .sa_handler = was != NULL ? was : SIG_DFL };
  }
  if (act != NULL) {
    bool runs = act->sa_handler != SIG_DFL && act->sa_handler != SIG_IGN;
    atomic_store(slot, runs ? (uintptr_t)act->sa_handler : 0);
  }
  return 0;
}

static inline int sigaltstack(const stack_t* ss, stack_t* old) {
  (void)ss;
  if (old != NULL) {
    *old = (stack_t){ .ss_flags = SS_DISABLE };
  }
  return 0;
}

static inline void (*eb_w32_signal(int sig, void (*handler)(int)))(int) {
  if (sig == SIGPIPE || sig == SIGBUS) {
    return SIG_DFL;
  }
  return signal(sig, handler);
}

static inline int eb_w32_raise(int sig) {
  if (sig == SIGPIPE || sig == SIGBUS) {
    return 0;
  }
  return raise(sig);
}

#define signal(sig, handler) eb_w32_signal(sig, handler)
#define raise(sig)           eb_w32_raise(sig)

#endif
