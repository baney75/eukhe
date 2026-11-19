// The <sys/select.h> of core.c on Windows: the IO loop's wait.
//
// Eukhe never reaches core.c's IO section: crates/eukhe-bend/c/shim.c
// renames main away and enters the runtime only through its heap and stack
// setup, io_str, corpus_eval and io_cstr, so io_loop never starts.
// The section compiles because it is in the same translation unit, so its
// calls map to Windows only where that is cheap and otherwise fail: select
// fails with ENOSYS (a UCRT descriptor is no Winsock socket), and so does
// the wake pipe's fcntl (c/win32/fcntl.h), which an IO loop calls first,
// so a loop would end at its start with its own fail-stop.
#ifndef EUKHE_BEND_SYS_SELECT_H
#define EUKHE_BEND_SYS_SELECT_H

#include <errno.h>

#define FD_SETSIZE 1024

typedef struct {
  unsigned long long bits[FD_SETSIZE / 64];
} fd_set;

struct timeval {
  long tv_sec;
  long tv_usec;
};

static inline int select(int top, fd_set* reads, fd_set* writes, fd_set* errors,
  struct timeval* wait) {
  (void)top;
  (void)reads;
  (void)writes;
  (void)errors;
  (void)wait;
  errno = ENOSYS;
  return -1;
}

#endif
