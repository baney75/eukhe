# eukhe: the proof of LAW approvals_runs_only_unasked_or_approved
# PROOF: the step's own test of its effects is the laws' (ro.within, by
# induction on the effects with LAG.gd.ok); within the auto set the step
# runs unasked; else its fate runs exactly when its question ended with a
# yes (ro.fate, a case per end).

import Base
import ../../bend/approvals.bend as Am
import ../../bend/tools.bend as Tl
import ../../laws/approvals/desk.bend as ApprovalsDesk
import ../../laws/approvals/route.bend as Approvals
import ./base/gate.bend as LAG

# the step's test of its effects is the laws' gt_all_allowed
def ro.within(es: List<&2, Nat>, +auto: List<&2, Nat>) -> {Tl.within(es, auto) == Approvals.gt_all_allowed(auto, es) : Bool}:
  match es:
    case Nil{}:
      {==}
    case +y <> rest:
      %LAG.gd.ok(auto, y) : {Bool.and(Bool.and(Tl.grantable(y), Tl.elem(auto, y)), Tl.within(rest, auto)) == Bool.and(_, Approvals.gt_all_allowed(auto, rest)) : Bool}
      %ro.within(rest, auto) : {Bool.and(Bool.and(Tl.grantable(y), Tl.elem(auto, y)), Tl.within(rest, auto)) == Bool.and(Bool.and(Tl.grantable(y), Tl.elem(auto, y)), _) : Bool}
      {==}

# a question's fate runs the step exactly when it ended with a yes
def ro.fate(+es: List<&2, Nat>, e: Am.End) -> {ApprovalsDesk.dy_runs(Am.fate(es, e)) == Approvals.gt_yes(e) : Bool}:
  match e:
    case Am.Answered{a, y}:
      match y:
        case True{}:
          {==}
        case False{}:
          {==}
    case Am.Unanswered{}:
      {==}

def ro.by(w: Bool, +es: List<&2, Nat>, +ds: List<&2, Am.Desk>) -> {Approvals.gt_runs(Am.gate.by(w, es, ds)) == Bool.or(w, Approvals.gt_yes(Am.Out.end(Am.route(ds)))) : Bool}:
  match w:
    case True{}:
      {==}
    case False{}:
      ro.fate(es, Am.Out.end(Am.route(ds)))

def Approvals.approvals_runs_only_unasked_or_approved(auto, es, ds):
  %ro.within(es, auto) : {Approvals.gt_runs(Am.gate.by(Tl.within(es, auto), es, ds)) == Bool.or(_, Approvals.gt_yes(Am.Out.end(Am.route(ds)))) : Bool}
  ro.by(Tl.within(es, auto), es, ds)
