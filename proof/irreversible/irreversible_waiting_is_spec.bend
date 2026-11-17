# eukhe: the proof of LAW irreversible_waiting_is_spec

import Base
import ../../bend/irreversible.bend as Ir
import ../../bend/ffi_irreversible.bend as FIr
import ../../bend/tools.bend as Tl
import ../../bend/approvals.bend as Am
import ../../bend/wire.bend as Wr
import ../../spec/irreversible/effects.bend as S
import ../../laws/irreversible.bend as Irreversible
import ../../laws/tools.bend as Tools
import ../../laws/wire.bend as Wire
import ../../laws/base.bend as LawsBase
import ./base.bend as IRB

def Irreversible.irreversible_waiting_is_spec(es):
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, st}:
          match st:
            case Ir.Waiting{}:
              %Irreversible.irreversible_waiting_is_spec(rest) : {Ir.waiting(Ir.Entry{i, Ir.Waiting{}} <> rest) == i <> _ : List<&2, Nat>}
              {==}
            case Ir.Taken{}:
              Irreversible.irreversible_waiting_is_spec(rest)
