// The UCRT's <fcntl.h> plus the F_SETFL and O_NONBLOCK of core.c's IO
// loop. A UCRT descriptor has no nonblocking mode, so fcntl fails with
// ENOSYS: an IO loop ends at its start with its own fail-stop (Eukhe opens
// none: c/win32/sys/select.h).
#ifndef EUKHE_BEND_FCNTL_H
#define EUKHE_BEND_FCNTL_H

#include_next <fcntl.h>

#include <errno.h>

#define F_GETFL    3
#define F_SETFL    4
#define O_NONBLOCK 0x10000000

static inline int fcntl(int fd, int cmd, ...) {
  (void)fd;
  (void)cmd;
  errno = ENOSYS;
  return -1;
}

#endif
