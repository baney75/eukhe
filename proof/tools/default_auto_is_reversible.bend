# eukhe: the proof of LAW default_auto_is_reversible

import Base
import ../../bend/approvals.bend as Am
import ../../bend/tools.bend as Tl
import ../../laws/tools.bend as Tools

# The grant
# ---------

def pa.default_auto(+c: Nat) -> {Tl.elem(Am.default_auto(), c) == Nat.is_lt(c, 3n) : Bool}:
  match c:
    case 0n:
      {==}
    case 1n+a:
      match a:
        case 0n:
          {==}
        case 1n+b:
          match b:
            case 0n:
              {==}
            case 1n+d:
              match d:
                case 0n:
                  {==}
                case 1n+e:
                  {==}

# (G5) The default grant: read, write, exec

def Tools.default_auto_is_reversible(c):
  pa.default_auto(c)
