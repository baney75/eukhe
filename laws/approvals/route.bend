# eukhe: the laws of area approvals, section route: where a question of
# the approval gate goes along a chain of sessions (bend/approvals.bend)

import Base
import ../../bend/approvals.bend as Am
import ./desk.bend as ApprovalsDesk

# ---- approvals ----

# LAWS (G5): each law is an open claim about the model in
# bend/approvals.bend (`route`, `fate`, `gate`); PROOF.bend proves them (as
# Approvals.<law>) with the lemmas of proof/approvals/. The desks of a
# chain of sessions, as the compiled core runs them, route a question
# exactly as this model does (laws/approvals/desk.bend,
# approvals_desk_is_route), so these laws hold for the chain Rust drives.
#
# Requirements (G5; source: crates/eukhe-daemon/src/shell_approval.rs, its
# module doc and its chain tests, and the header of bend/approvals.bend.
# The chain is eukhe's own requirement, stated by no outside source):
# G5.1 a question goes up the chain, from the session that asks to its
#      root, until a session's clients (the TUI, the dashboard, the phone
#      gateway, which asks over Signal) answer: the nearest one wins, and
#      the end names that session (child_answered_by_root,
#      nearest_answer_wins).
# G5.2 when no session up to the root answers, the question is unanswered
#      (a denial), and only then (quiet_is_unanswered,
#      unanswered_only_when_quiet).
# G5.5 a step runs unasked exactly when each of its effects is an effect
#      (codes 0 to 6: crates/eukhe-types/src/shell.rs `ShellEffect::code`;
#      every effect is grantable, bend/tools.bend `grantable`) that the
#      session's auto set holds; any other step's question goes its route
#      and the step fares by its end (approvals_disallowed_effect_asks,
#      approvals_asks_only_for_a_disallowed_effect). What the default auto
#      set holds: laws/tools.bend default_auto_is_reversible.
# G5.6 a step runs only unasked or on a yes: a question no one answered
#      is never a yes, on any surface (spec/README.md I5;
#      approvals_runs_only_unasked_or_approved).

# Terms
# -----

# the session's auto set runs the effect x unasked: x is an effect (a code
# below 7) and the auto set holds it
def gt_allowed(+auto: List<&2, Nat>, +x: Nat) -> Bool:
  Bool.and(Nat.is_lt(x, 7n), ApprovalsDesk.sp_has(auto, x))

# the effects es hold one the auto set does not run unasked: es is some
# pre, then such an x, then some post
def gt_has_disallowed(+auto: List<&2, Nat>, +es: List<&2, Nat>) -> Type:
  &pre: List<&2, Nat> -> &x: Nat -> &post: List<&2, Nat> -> ({es == List.append(&2, Nat, pre, x <> post) : List<&2, Nat>} & {gt_allowed(auto, x) == False{} : Bool})

# (G5.1, G5.2) A child's question reaches its root's user
# --------------------------------------------------------
# A chain lists the sessions from the one that asks (a subagent, or a
# root) up to its root.

# LAW child_answered_by_root: when no session below the root has clients that answer, a
# subagent's question reaches the root's clients, and their answer is the
# verdict
law child_answered_by_root:
  for +below: List<&2, Am.Desk>
  for quiet: {Am.quiet(below) == True{} : Bool}
  for +yes: Bool
  {Am.Out.end(Am.route(List.append(&2, Am.Desk, below, [Am.Says{yes}]))) == Am.Answered{List.length(&2, Am.Desk, below), yes} : Am.End}

# LAW nearest_answer_wins: the first session up the chain whose clients answer gives the
# verdict, whatever the sessions above it
law nearest_answer_wins:
  for +below: List<&2, Am.Desk>
  for quiet: {Am.quiet(below) == True{} : Bool}
  for +yes: Bool
  for +above: List<&2, Am.Desk>
  {Am.Out.end(Am.route(List.append(&2, Am.Desk, below, Am.Says{yes} <> above))) == Am.Answered{List.length(&2, Am.Desk, below), yes} : Am.End}

# LAW unanswered_only_when_quiet: a question is unanswered (a denial) only when no session of the
# whole chain has clients that answer
law unanswered_only_when_quiet:
  for +ds: List<&2, Am.Desk>
  for none: {Am.Out.end(Am.route(ds)) == Am.Unanswered{} : Am.End}
  {Am.quiet(ds) == True{} : Bool}

# LAW quiet_is_unanswered: and then it is unanswered: no session invents an answer
law quiet_is_unanswered:
  for +ds: List<&2, Am.Desk>
  for quiet: {Am.quiet(ds) == True{} : Bool}
  {Am.Out.end(Am.route(ds)) == Am.Unanswered{} : Am.End}

# (G5.5) Which steps ask
# ----------------------

# LAW approvals_disallowed_effect_asks (G5.5): a step with an effect x
# that the session's auto set does not run unasked (x is no effect code
# below 7, or the auto set lacks it), wherever x sits among the step's
# effects (pre, then x, then post), is asked: its question goes its route
# along any chain ds, and the step fares by how the question ended
# EXCLUDES: a gate that runs unasked a step the session's auto set does
#   not allow (one effect checked, the last one only, a fixed or
#   remembered auto set, an auto set read for another session, an effect
#   past the known codes let through)
law approvals_disallowed_effect_asks:
  for +auto: List<&2, Nat>
  for +pre: List<&2, Nat>
  for +x: Nat
  for +post: List<&2, Nat>
  for +ds: List<&2, Am.Desk>
  for no: {gt_allowed(auto, x) == False{} : Bool}
  {Am.gate(auto, List.append(&2, Nat, pre, x <> post), ds) == Am.Routed{Am.route(ds), Am.fate(List.append(&2, Nat, pre, x <> post), Am.Out.end(Am.route(ds)))} : Am.Gated}

# LAW approvals_asks_only_for_a_disallowed_effect (G5.5): a step that is
# asked (its gate routes it, with any route and fate) has an effect that
# the session's auto set does not run unasked: its effects are some pre,
# then such an x, then some post. So a step whose every effect is one the
# auto set holds runs unasked, along any chain
# EXCLUDES: a gate that asks about a step the auto set allows (a question
#   for a read the user granted, an empty step asked)
law approvals_asks_only_for_a_disallowed_effect:
  for +auto: List<&2, Nat>
  for +es: List<&2, Nat>
  for +ds: List<&2, Am.Desk>
  for +o: Am.Out
  for +f: Am.Fate
  for asked: {Am.gate(auto, es, ds) == Am.Routed{o, f} : Am.Gated}
  gt_has_disallowed(auto, es)

# (G5.6) A step runs only unasked or on a yes
# ------------------------------------------

# every effect of es is one the auto set runs unasked
def gt_all_allowed(+auto: List<&2, Nat>, es: List<&2, Nat>) -> Bool:
  match es:
    case Nil{}:
      True{}
    case x <> rest:
      Bool.and(gt_allowed(auto, x), gt_all_allowed(auto, rest))

# the question ended with a session's clients answering yes
def gt_yes(e: Am.End) -> Bool:
  match e:
    case Am.Answered{a, y}:
      y
    case Am.Unanswered{}:
      False{}

# the gate lets the step run
def gt_runs(g: Am.Gated) -> Bool:
  match g:
    case Am.Unasked{}:
      True{}
    case Am.Routed{o, f}:
      ApprovalsDesk.dy_runs(f)

# LAW approvals_runs_only_unasked_or_approved (G5.6): along any chain ds,
# under any auto set, a step with any effects runs exactly when the auto
# set runs each of its effects unasked, or its question ends with a
# session's clients (the phone gateway among them) answering yes. A
# question no one answers is never a yes, on any surface
# EXCLUDES: a session that answers yes for a user who did not (a rule by
#   effect, an unanswered question that runs), a yes that does not run,
#   an allowed step that is asked
law approvals_runs_only_unasked_or_approved:
  for +auto: List<&2, Nat>
  for +es: List<&2, Nat>
  for +ds: List<&2, Am.Desk>
  {gt_runs(Am.gate(auto, es, ds)) == Bool.or(gt_all_allowed(auto, es), gt_yes(Am.Out.end(Am.route(ds)))) : Bool}
