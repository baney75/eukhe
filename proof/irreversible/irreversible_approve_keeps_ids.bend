# eukhe: the proof of LAW irreversible_approve_keeps_ids

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

def Irreversible.irreversible_approve_keeps_ids(w, es):
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, +st}:
          +a = Ir.approve(w, rest)
          %Equal.sym(List<&2, Nat>, Irreversible.ir_ids(Ir.Approved.ledger(Ir.approve.cons(i, st, Ir.approve.take(st, w, i), a))), i <> Irreversible.ir_ids(Ir.Approved.ledger(a)), IRB.il.cons_ids(i, st, Ir.approve.take(st, w, i), a)) : {_ == i <> Irreversible.ir_ids(rest) : List<&2, Nat>}
          %Equal.sym(List<&2, Nat>, Irreversible.ir_ids(Ir.Approved.ledger(a)), Irreversible.ir_ids(rest), Irreversible.irreversible_approve_keeps_ids(w, rest)) : {i <> _ == i <> Irreversible.ir_ids(rest) : List<&2, Nat>}
          {==}
