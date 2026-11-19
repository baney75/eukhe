// The <poll.h> of core.c on Windows: the event bits its IO loop tags work
// with. The loop waits in select (c/win32/sys/select.h), so poll itself is
// not here.
#ifndef EUKHE_BEND_POLL_H
#define EUKHE_BEND_POLL_H

#define POLLIN  0x0001
#define POLLOUT 0x0004
#define POLLERR 0x0008
#define POLLHUP 0x0010

#endif
