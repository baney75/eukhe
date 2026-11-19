// The <netinet/in.h> of core.c on Windows: the IPv4 address its IO
// section parses (c/win32/sys/select.h).
#ifndef EUKHE_BEND_NETINET_IN_H
#define EUKHE_BEND_NETINET_IN_H

#include <stdint.h>

#include "../sys/socket.h"

typedef uint16_t in_port_t;

struct in_addr {
  uint32_t s_addr;
};

struct sockaddr_in {
  sa_family_t    sin_family;
  in_port_t      sin_port;
  struct in_addr sin_addr;
  char           sin_zero[8];
};

#endif
