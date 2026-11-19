// The UCRT's <stdio.h> plus ftrylockfile, which err_trap calls before it
// flushes stdout. The UCRT has no try-lock of a FILE, and a fault may
// strike while the faulting thread holds stdout's lock, so it answers
// busy: the trap skips the flush and reports the fault on stderr.
#ifndef EUKHE_BEND_STDIO_H
#define EUKHE_BEND_STDIO_H

#include_next <stdio.h>

static inline int ftrylockfile(FILE* file) {
  (void)file;
  return -1;
}

#endif
