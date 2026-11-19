// The <unistd.h> of core.c on Windows: the UCRT's read, write and close
// (<io.h>), _exit (<stdlib.h>), ssize_t, sysconf for the processor count,
// pipe over the UCRT's _pipe, and readlink of /proc/self/exe (the one link
// the runtime reads) over GetModuleFileNameA.
#ifndef EUKHE_BEND_UNISTD_H
#define EUKHE_BEND_UNISTD_H

#include <errno.h>
#include <fcntl.h>
#include <io.h>
#include <stdlib.h>
#include <string.h>

#include "eb_win32.h"

typedef intptr_t ssize_t;

#define _SC_NPROCESSORS_ONLN 84

static inline long sysconf(int name) {
  if (name != _SC_NPROCESSORS_ONLN) {
    errno = EINVAL;
    return -1;
  }
  return (long)GetActiveProcessorCount(EB_W32_ALL_PROCESSOR_GROUPS);
}

// A binary pipe of the UCRT's descriptors (the IO loop's wake pipe, which
// Eukhe never opens: c/win32/sys/select.h).
static inline int pipe(int fds[2]) {
  return _pipe(fds, 4096, _O_BINARY);
}

static inline ssize_t readlink(const char* path, char* buf, size_t size) {
  if (strcmp(path, "/proc/self/exe") != 0 || size == 0 || size > 0xFFFFFFFFu) {
    errno = EINVAL;
    return -1;
  }
  uint32_t n = GetModuleFileNameA(NULL, buf, (uint32_t)size);
  if (n == 0) {
    errno = ENOENT;
    return -1;
  }
  return (ssize_t)n;
}

#endif
