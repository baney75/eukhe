# eukhe: the proof of LAW irreversible_approve_runs_chosen

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

def Irreversible.irreversible_approve_runs_chosen(w, es):
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, st}:
          match st:
            case Ir.Waiting{}:
              +a = Ir.approve(w, rest)
              %Equal.sym(List<&2, Nat>, Ir.Approved.run(Ir.approve.cons(i, Ir.Waiting{}, Ir.picks(w, i), a)), S.keep(Ir.picks(w, i), i, Ir.Approved.run(a)), IRB.il.cons_run(i, Ir.Waiting{}, Ir.picks(w, i), a)) : {_ == S.chosen(w, i <> S.waiting(rest)) : List<&2, Nat>}
              %Equal.sym(Bool, Ir.picks(w, i), S.selects(w, i), IRB.irb.picks(w, i)) : {S.keep(_, i, Ir.Approved.run(a)) == S.chosen(w, i <> S.waiting(rest)) : List<&2, Nat>}
              %Equal.sym(List<&2, Nat>, Ir.Approved.run(a), S.chosen(w, S.waiting(rest)), Irreversible.irreversible_approve_runs_chosen(w, rest)) : {S.keep(S.selects(w, i), i, _) == S.chosen(w, i <> S.waiting(rest)) : List<&2, Nat>}
              {==}
            case Ir.Taken{}:
              Equal.trans(List<&2, Nat>, Ir.Approved.run(Ir.approve.cons(i, Ir.Taken{}, False{}, Ir.approve(w, rest))), Ir.Approved.run(Ir.approve(w, rest)), S.chosen(w, S.waiting(rest)), IRB.il.cons_run(i, Ir.Taken{}, False{}, Ir.approve(w, rest)), Irreversible.irreversible_approve_runs_chosen(w, rest))
