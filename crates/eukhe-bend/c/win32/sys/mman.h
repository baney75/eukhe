// The anonymous mappings of core.c on Windows.
//
// mmap(MAP_NORESERVE) is VirtualAlloc(MEM_RESERVE): address space only, as
// the 8 GiB corpus and the 2 GiB machine stacks need. Windows commits no
// page on touch by itself, so a vectored exception handler does: an access
// violation inside one of these reservations, on a page still reserved,
// commits the 1 MiB around it read-write (the protection core.c maps with)
// and resumes the faulting access. A commit the system refuses is out of
// memory, which ends the process as a refused overcommit does on Linux.
//
// mprotect(PROT_NONE) commits its pages no-access: a guard. An access
// violation on a committed page of a reservation (a guard hit, above all
// the machine stack's) runs the SIGSEGV handler sigaction recorded
// (c/win32/signal.h), core.c's err_trap, which reports the fault and ends
// the process; with none recorded the fault goes on to the default crash.
// Faults outside these reservations are not this handler's.
//
// A hint address is taken as is or the call fails, so a hint never moves
// (corpus_at in core.c tries another). munmap releases a whole mapping, as
// Windows releases only whole reservations: core.c unmaps only a mapping
// it just made. A file mapping fails with ENODEV: only the CUDA build maps
// one (gpu_load), and the Windows build has none.
#ifndef EUKHE_BEND_SYS_MMAN_H
#define EUKHE_BEND_SYS_MMAN_H

#include <errno.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <unistd.h>

#include "../eb_win32.h"
#include "../signal.h"

#define PROT_NONE     0
#define PROT_READ     1
#define PROT_WRITE    2
#define PROT_EXEC     4
#define MAP_SHARED    0x01
#define MAP_PRIVATE   0x02
#define MAP_ANON      0x20
#define MAP_ANONYMOUS MAP_ANON
#define MAP_NORESERVE 0x4000
#define MAP_FAILED    ((void*)-1)

// The reservations' bases (0: a free slot). core.c maps a handful: the
// corpus and its doublings, a machine stack per lane.
#define EB_W32_MAPS 64

static _Atomic uintptr_t eb_w32_maps[EB_W32_MAPS];
static atomic_flag       eb_w32_fault_handler_added = ATOMIC_FLAG_INIT;

// The commit-on-touch step.
#define EB_W32_COMMIT_CHUNK ((uintptr_t)1 << 20)

static inline bool eb_w32_is_map(const void* base) {
  for (size_t i = 0; i < EB_W32_MAPS; i += 1) {
    if (base != NULL && atomic_load(&eb_w32_maps[i]) == (uintptr_t)base) {
      return true;
    }
  }
  return false;
}

static inline bool eb_w32_map_add(void* base) {
  for (size_t i = 0; i < EB_W32_MAPS; i += 1) {
    uintptr_t free_slot = 0;
    if (atomic_compare_exchange_strong(&eb_w32_maps[i], &free_slot, (uintptr_t)base)) {
      return true;
    }
  }
  return false;
}

static inline bool eb_w32_map_drop(void* base) {
  for (size_t i = 0; i < EB_W32_MAPS; i += 1) {
    uintptr_t at = (uintptr_t)base;
    if (atomic_compare_exchange_strong(&eb_w32_maps[i], &at, 0)) {
      return true;
    }
  }
  return false;
}

static long __stdcall eb_w32_fault(eb_w32_exception_pointers* fault) {
  eb_w32_exception_record* record = fault->record;
  if (record->code != EB_W32_EXCEPTION_ACCESS_VIOLATION || record->parameters < 2) {
    return EB_W32_EXCEPTION_CONTINUE_SEARCH;
  }
  // information[0]: 0 read, 1 write, 8 execute; information[1]: the address.
  uintptr_t      at   = record->information[1];
  eb_w32_memory_info info = { 0 };
  if (VirtualQuery((const void*)at, &info, sizeof info) != sizeof info
    || !eb_w32_is_map(info.allocation_base)) {
    return EB_W32_EXCEPTION_CONTINUE_SEARCH;
  }
  if (info.state == EB_W32_MEM_RESERVE) {
    uintptr_t lo  = (uintptr_t)info.base;
    uintptr_t hi  = lo + info.region_size;
    uintptr_t from = at & ~(EB_W32_COMMIT_CHUNK - 1);
    uintptr_t to   = from + EB_W32_COMMIT_CHUNK;
    from = from < lo ? lo : from;
    to   = to > hi ? hi : to;
    if (VirtualAlloc((void*)from, to - from, EB_W32_MEM_COMMIT, EB_W32_PAGE_READWRITE) == NULL) {
      static const char line[] = "bend: out of memory (the system refused a commit)\n";
      if (write(2, line, sizeof line - 1)) {}
      _exit(1);
    }
    return EB_W32_EXCEPTION_CONTINUE_EXECUTION;
  }
  if (info.protect == EB_W32_PAGE_READWRITE && record->information[0] != 8) {
    // Another thread committed the page between the fault and the query.
    return EB_W32_EXCEPTION_CONTINUE_EXECUTION;
  }
  void (*trap)(int) = eb_w32_trap_handler(SIGSEGV);
  if (trap != NULL) {
    trap(SIGSEGV);
  }
  return EB_W32_EXCEPTION_CONTINUE_SEARCH;
}

static inline uint32_t eb_w32_page_protection(int prot) {
  return prot == PROT_NONE ? EB_W32_PAGE_NOACCESS
    : (prot & PROT_EXEC) != 0 ? EB_W32_PAGE_EXECUTE_READWRITE
    : (prot & PROT_WRITE) != 0 ? EB_W32_PAGE_READWRITE
    : EB_W32_PAGE_READONLY;
}

static inline void* mmap(void* at, size_t size, int prot, int flags, int fd, intptr_t off) {
  (void)off;
  if ((flags & MAP_ANON) == 0 || fd != -1) {
    errno = ENODEV;
    return MAP_FAILED;
  }
  if (size == 0) {
    errno = EINVAL;
    return MAP_FAILED;
  }
  bool     lazy = (flags & MAP_NORESERVE) != 0;
  uint32_t type = lazy ? EB_W32_MEM_RESERVE : EB_W32_MEM_RESERVE | EB_W32_MEM_COMMIT;
  void*    p    = VirtualAlloc(at, size, type, lazy ? EB_W32_PAGE_NOACCESS : eb_w32_page_protection(prot));
  if (p == NULL) {
    errno = ENOMEM;
    return MAP_FAILED;
  }
  if (!eb_w32_map_add(p)) {
    VirtualFree(p, 0, EB_W32_MEM_RELEASE);
    errno = ENOMEM;
    return MAP_FAILED;
  }
  if (!atomic_flag_test_and_set(&eb_w32_fault_handler_added)) {
    AddVectoredExceptionHandler(1, eb_w32_fault);
  }
  return p;
}

static inline int munmap(void* at, size_t size) {
  (void)size;
  if (!eb_w32_map_drop(at)) {
    errno = EINVAL;
    return -1;
  }
  return VirtualFree(at, 0, EB_W32_MEM_RELEASE) ? 0 : (errno = EINVAL, -1);
}

static inline int mprotect(void* at, size_t size, int prot) {
  uint32_t page = eb_w32_page_protection(prot);
  uint32_t old  = 0;
  if (VirtualAlloc(at, size, EB_W32_MEM_COMMIT, page) == NULL
    || !VirtualProtect(at, size, page, &old)) {
    errno = ENOMEM;
    return -1;
  }
  return 0;
}

#endif
