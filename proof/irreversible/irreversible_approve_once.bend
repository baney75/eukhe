# eukhe: the proof of LAW irreversible_approve_once

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

def Irreversible.irreversible_approve_once(w, es):
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, st}:
          match st:
            case Ir.Waiting{}:
              Equal.trans(List<&2, Nat>, Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve.cons(i, Ir.Waiting{}, Ir.picks(w, i), Ir.approve(w, rest))))), Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve(w, rest)))), [], IRB.il.once_waiting(w, i, Ir.picks(w, i), {==}, Ir.approve(w, rest)), Irreversible.irreversible_approve_once(w, rest))
            case Ir.Taken{}:
              Equal.trans(List<&2, Nat>, Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve.cons(i, Ir.Taken{}, False{}, Ir.approve(w, rest))))), Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve(w, rest)))), [], IRB.il.once_taken(w, i, Ir.approve(w, rest)), Irreversible.irreversible_approve_once(w, rest))
