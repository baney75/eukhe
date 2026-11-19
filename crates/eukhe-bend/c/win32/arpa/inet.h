// The <arpa/inet.h> of core.c on Windows: htons and inet_pton(AF_INET) in
// plain C, so the IO section's address parsing needs no Winsock
// (c/win32/sys/select.h).
#ifndef EUKHE_BEND_ARPA_INET_H
#define EUKHE_BEND_ARPA_INET_H

#include <errno.h>
#include <stdint.h>
#include <string.h>

#include "../netinet/in.h"

// Windows runs little-endian on every target Rust builds for it.
static inline uint16_t htons(uint16_t v) {
  return (uint16_t)(v << 8 | v >> 8);
}

// Dotted-quad IPv4 only, as POSIX inet_pton(AF_INET): four decimal parts
// of 0 to 255; 1 on success, 0 on bad text, -1 for another family.
static inline int inet_pton(int family, const char* text, void* out) {
  if (family != AF_INET) {
    errno = EAFNOSUPPORT;
    return -1;
  }
  uint8_t     octets[4] = { 0 };
  const char* p         = text;
  for (int i = 0; i < 4; i += 1) {
    unsigned part   = 0;
    int      digits = 0;
    while (*p >= '0' && *p <= '9' && digits < 4) {
      part    = part * 10 + (unsigned)(*p - '0');
      digits += 1;
      p      += 1;
    }
    if (digits == 0 || digits > 3 || part > 255 || *p != (i < 3 ? '.' : '\0')) {
      return 0;
    }
    octets[i] = (uint8_t)part;
    p        += i < 3;
  }
  struct in_addr addr = { 0 };
  memcpy(&addr.s_addr, octets, sizeof octets);
  *(struct in_addr*)out = addr;
  return 1;
}

#endif
