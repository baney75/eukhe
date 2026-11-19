// The <sys/socket.h> of core.c on Windows: the address family its IO
// section names (it makes no socket call; c/win32/sys/select.h).
#ifndef EUKHE_BEND_SYS_SOCKET_H
#define EUKHE_BEND_SYS_SOCKET_H

#include <stdint.h>

#define AF_INET 2

typedef uint16_t sa_family_t;
typedef int      socklen_t;

struct sockaddr {
  sa_family_t sa_family;
  char        sa_data[14];
};

#endif
