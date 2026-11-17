# eukhe: the proof of LAW irreversible_runs_only_approved

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

def Irreversible.irreversible_runs_only_approved(ops, es):
  match ops:
    case Nil{}:
      {==}
    case op <> +rest:
      match op:
        case Ir.Hold{+j}:
          %Equal.sym(Bool, S.all_approved(Ir.Hold{j} <> rest, Ir.Hist.ran(Ir.run(rest, Ir.hold(es, j)))), S.all_approved(rest, Ir.Hist.ran(Ir.run(rest, Ir.hold(es, j)))), IRB.ih.all_hold(j, rest, Ir.Hist.ran(Ir.run(rest, Ir.hold(es, j))))) : {_ == True{} : Bool}
          Irreversible.irreversible_runs_only_approved(rest, Ir.hold(es, j))
        case Ir.Approve{+w}:
          +a = Ir.approve(w, es)
          +tail = Ir.Hist.ran(Ir.run(rest, Ir.Approved.ledger(a)))
          %Equal.sym(List<&2, Nat>, Ir.Hist.ran(Ir.hist.after(Ir.Approved.run(a), Ir.run(rest, Ir.Approved.ledger(a)))), List.append(&2, Nat, Ir.Approved.run(a), tail), IRB.ih.ran_after(Ir.Approved.run(a), Ir.run(rest, Ir.Approved.ledger(a)))) : {S.all_approved(Ir.Approve{w} <> rest, _) == True{} : Bool}
          %Equal.sym(Bool, S.all_approved(Ir.Approve{w} <> rest, List.append(&2, Nat, Ir.Approved.run(a), tail)), Bool.and(S.all_approved(Ir.Approve{w} <> rest, Ir.Approved.run(a)), S.all_approved(Ir.Approve{w} <> rest, tail)), IRB.ih.all_app(Ir.Approve{w} <> rest, Ir.Approved.run(a), tail)) : {_ == True{} : Bool}
          %Equal.sym(List<&2, Nat>, Ir.Approved.run(Ir.approve(w, es)), S.chosen(w, S.waiting(es)), Irreversible.irreversible_approve_runs_chosen(w, es)) : {Bool.and(S.all_approved(Ir.Approve{w} <> rest, _), S.all_approved(Ir.Approve{w} <> rest, tail)) == True{} : Bool}
          %Equal.sym(Bool, S.all_approved(Ir.Approve{w} <> rest, S.chosen(w, S.waiting(es))), True{}, IRB.ih.chosen(w, rest, S.waiting(es))) : {Bool.and(_, S.all_approved(Ir.Approve{w} <> rest, tail)) == True{} : Bool}
          IRB.ih.lift(w, rest, tail, Irreversible.irreversible_runs_only_approved(rest, Ir.Approved.ledger(a)))
