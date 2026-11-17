# eukhe: the laws of area irreversible

import Base
import ../bend/irreversible.bend as Ir
import ../bend/ffi_irreversible.bend as FIr
import ../bend/approvals.bend as Am
import ../bend/wire.bend as Wr
import ../spec/irreversible/effects.bend as S
import ./base.bend as LawsBase

# (I1-I6, I9) Effects outside the grant wait for the principal's approval
# -----------------------------------------------------------------------
# spec/README.md I1-I6, I9 (the owner's decision of 2026-11-16:
# silence is never a yes).
# bend/irreversible.bend decides what becomes of an effect (its codes, the
# grant `shell.autoEffects`, the mode) and keeps the ledger of the effects
# that wait; it runs as the compiled core by bend/ffi_irreversible.bend
# (eukhe-bend rules/irreversible.rs), asked by the gates of eukhe-core (a
# `bash` step, a document tool's access, a `python` cell, an MCP call) and
# by the session's `eukhe.effects` ledger. spec/irreversible/effects.bend
# restates the rule (S1-S6). Every input is free: any codes (past the
# vocabulary too), any grant, either mode, any ledger, any history.
#
# R1 (I1): an effect runs unasked exactly when each of its codes is a known
#     code the grant holds; under the shipped grant exactly when each is
#     read, write or exec; one of unknown kind never runs unasked.
# R2 (I2): an effect in the grant runs, in every mode.
# R3 (I3, I9): any other effect waits in the default mode and is asked in
#     watch mode; it never runs unasked.
# R4 (I5): a yes runs it, a no refuses it, no answer leaves it waiting.
# R5 (I3, I6): recording an effect adds exactly its id, last, to the
#     waiting list; recording an id the ledger holds changes nothing; the
#     end-of-run list is exactly the waiting ids, in recorded order.
# R6 (I4): an approval runs exactly the waiting ids it selects, in ledger
#     order, leaves exactly the others waiting, drops and adds no entry;
#     the same approval again runs nothing.
# R7 (I4, I5): over any history of a ledger, nothing runs without an
#     approval, and an id runs only when an approval of the history
#     selects it.
# R8: the entry answers each request Rust writes with the model's
#     decision, reads exactly the request, and answers any other input
#     with the exact error form `E irreversible: malformed request`.
#
# Not pinned here: that Rust asks the route at every gate, records a
# waiting effect before it answers the agent, runs what an approval
# returns exactly once and stores the ledger the core returns: the Rust
# tests (eukhe-core, eukhe-daemon irreversible_e2e) cover it.

# Spec helpers
# ------------

def ir_runs(r: Ir.Route) -> Bool:
  match r:
    case Ir.Runs{}:
      True{}
    case Ir.Waits{}:
      False{}
    case Ir.Asks{}:
      False{}

def ir_mode_tag(m: Ir.Mode) -> Nat:
  match m:
    case Ir.End{}:
      0n
    case Ir.Watch{}:
      1n

def ir_said_tag(s: Ir.Said) -> Nat:
  match s:
    case Ir.Yes{}:
      0n
    case Ir.No{}:
      1n
    case Ir.Silent{}:
      2n

def ir_w_ns(xs: List<&2, Nat>, rest: String) -> String:
  Wr.put_list(~Nat, ~Wr.put_nat, xs, rest)

def ir_st_tag(st: Ir.St) -> Nat:
  match st:
    case Ir.Waiting{}:
      0n
    case Ir.Taken{}:
      1n

# R8: a ledger is one list of Nats: each entry's id, then its state
def ir_flat(es: List<&2, Ir.Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      match e:
        case Ir.Entry{i, st}:
          i <> (ir_st_tag(st) <> ir_flat(rest))

def ir_w_ledger(es: List<&2, Ir.Entry>, rest: String) -> String:
  ir_w_ns(ir_flat(es), rest)

# R8: a list of Nats the wire carries: its count and every item fit
def ir_ns_fit(+xs: List<&2, Nat>) -> Bool:
  Bool.and(LawsBase.wire_fits(List.length(&2, Nat, xs)), LawsBase.wire_nats_fit(xs))

# R8: a selection the wire carries
def ir_which_fit(w: Ir.Which) -> Bool:
  match w:
    case Ir.All{}:
      True{}
    case Ir.Ids{ks}:
      ir_ns_fit(ks)

def ir_w_which(w: Ir.Which, rest: String) -> String:
  match w:
    case Ir.All{}:
      Wr.put_nat(0n, rest)
    case Ir.Ids{ks}:
      Wr.put_nat(1n, ir_w_ns(ks, rest))

# R8: the spelling of each request
def ir_w_route(auto: List<&2, Nat>, effs: List<&2, Nat>, m: Ir.Mode, rest: String) -> String:
  Wr.put_nat(0n, ir_w_ns(auto, ir_w_ns(effs, Wr.put_nat(ir_mode_tag(m), rest))))

def ir_w_settle(s: Ir.Said, rest: String) -> String:
  Wr.put_nat(1n, Wr.put_nat(ir_said_tag(s), rest))

def ir_w_hold(es: List<&2, Ir.Entry>, i: Nat, rest: String) -> String:
  Wr.put_nat(2n, ir_w_ledger(es, Wr.put_nat(i, rest)))

def ir_w_approve(w: Ir.Which, es: List<&2, Ir.Entry>, rest: String) -> String:
  Wr.put_nat(3n, ir_w_which(w, ir_w_ledger(es, rest)))

def ir_w_waiting(es: List<&2, Ir.Entry>, rest: String) -> String:
  Wr.put_nat(4n, ir_w_ledger(es, rest))

# R8: the spelling of each reply
def ir_w_route_reply(r: Ir.Route) -> String:
  match r:
    case Ir.Runs{}:
      Wr.put_nat(0n, "")
    case Ir.Waits{}:
      Wr.put_nat(1n, "")
    case Ir.Asks{}:
      Wr.put_nat(2n, "")

def ir_w_fate_reply(f: Ir.Fate) -> String:
  match f:
    case Ir.Ran{}:
      Wr.put_nat(0n, "")
    case Ir.Refused{}:
      Wr.put_nat(1n, "")
    case Ir.Held{}:
      Wr.put_nat(2n, "")

def ir_ids(es: List<&2, Ir.Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      match e:
        case Ir.Entry{i, st}:
          i <> ir_ids(rest)

# Laws: the route (R1-R3)
# -----------------------

# LAW irreversible_route_is_spec (R1-R3): what becomes of an effect is the
# spec's for every codes, grant and mode
# EXCLUDES: a route keyed on the first code only, a grant that admits a code
#   past 6, watch and end swapped, a default
law irreversible_route_is_spec:
  for +effs: List<&2, Nat>
  for +auto: List<&2, Nat>
  for m: Ir.Mode
  {Ir.route(effs, auto, m) == S.route(effs, auto, m) : Ir.Route}

# LAW irreversible_default_runs_exactly_reversible (R1): under the shipped
# grant an effect runs unasked exactly when each of its codes is read,
# write or exec, in either mode
# EXCLUDES: a shipped grant that holds net, outside, protected or secret
#   (the old default of every effect, where a `git push` ran unasked), one
#   that drops exec (builds and tests asked), a code past 6 admitted
law irreversible_default_runs_exactly_reversible:
  for +effs: List<&2, Nat>
  for m: Ir.Mode
  {ir_runs(Ir.route(effs, Am.default_auto(), m)) == S.reversible(effs) : Bool}

# LAW irreversible_unknown_never_runs (R1): an effect with a code of unknown
# kind (past 6) anywhere among its codes never runs unasked, whatever the
# grant holds and in either mode
# EXCLUDES: a grant that lists an unknown code and so runs it, an unknown
#   code ignored when the others are granted
law irreversible_unknown_never_runs:
  for +effs: List<&2, Nat>
  for +auto: List<&2, Nat>
  for m: Ir.Mode
  for +c: Nat
  for has: {S.has(effs, c) == True{} : Bool}
  for u: {S.below(7n, c) == False{} : Bool}
  {ir_runs(Ir.route(effs, auto, m)) == False{} : Bool}

# LAW irreversible_granted_runs (R2): an effect in the grant runs in every
# mode, whatever waits (the route reads no ledger)
# EXCLUDES: watch mode asking for a granted effect, a granted effect held
#   behind a waiting one
law irreversible_granted_runs:
  for +effs: List<&2, Nat>
  for +auto: List<&2, Nat>
  for m: Ir.Mode
  for g: {S.in_grant(effs, auto) == True{} : Bool}
  {Ir.route(effs, auto, m) == Ir.Runs{} : Ir.Route}

# LAW irreversible_outside_waits (R3): in the default mode an effect outside
# the grant waits: it does not run and no one is asked
# EXCLUDES: the old prompt mid-run, an effect outside the grant run unasked
#   (the phone gateway's yes to an unprotected step)
law irreversible_outside_waits:
  for +effs: List<&2, Nat>
  for +auto: List<&2, Nat>
  for g: {S.in_grant(effs, auto) == False{} : Bool}
  {Ir.route(effs, auto, Ir.End{}) == Ir.Waits{} : Ir.Route}

# LAW irreversible_watch_asks (R3): in watch mode an effect outside the
# grant is asked
# EXCLUDES: watch mode that holds silently or runs
law irreversible_watch_asks:
  for +effs: List<&2, Nat>
  for +auto: List<&2, Nat>
  for g: {S.in_grant(effs, auto) == False{} : Bool}
  {Ir.route(effs, auto, Ir.Watch{}) == Ir.Asks{} : Ir.Route}

# LAW irreversible_settle_is_spec (R4): a yes runs, a no refuses, no answer
# waits
# EXCLUDES: silence read as a yes (the effect runs) or as a no (it is lost
#   instead of waiting), a no that leaves it waiting
law irreversible_settle_is_spec:
  for s: Ir.Said
  {Ir.settle(s) == S.settle(s) : Ir.Fate}

# Laws: the ledger (R5, R6)
# -------------------------

# LAW irreversible_waiting_is_spec (R5): the end-of-run list is exactly the
# waiting ids, in recorded order
# EXCLUDES: a list that omits a waiting effect, names a done one, or
#   reorders them
law irreversible_waiting_is_spec:
  for es: List<&2, Ir.Entry>
  {Ir.waiting(es) == S.waiting(es) : List<&2, Nat>}

# LAW irreversible_hold_appends (R5): recording a new id adds exactly it,
# last, to the waiting list
# EXCLUDES: a record that drops or reorders the earlier waiting effects, or
#   records the id done
law irreversible_hold_appends:
  for +es: List<&2, Ir.Entry>
  for +i: Nat
  for new: {S.has(ir_ids(es), i) == False{} : Bool}
  {S.waiting(Ir.hold(es, i)) == List.append(&2, Nat, S.waiting(es), [i]) : List<&2, Nat>}

# LAW irreversible_hold_known_keeps (R5): recording an id the ledger holds
# changes nothing (a redelivered record; ids are never reused)
# EXCLUDES: a done effect made waiting again by a second record, a
#   duplicate entry
law irreversible_hold_known_keeps:
  for +es: List<&2, Ir.Entry>
  for +i: Nat
  for known: {S.has(ir_ids(es), i) == True{} : Bool}
  {Ir.hold(es, i) == es : List<&2, Ir.Entry>}

# LAW irreversible_approve_runs_chosen (R6): an approval runs exactly the
# waiting ids it selects, in ledger order
# EXCLUDES: an approval that runs a done effect again, skips a selected
#   waiting one, runs one it did not select, or runs them out of order
law irreversible_approve_runs_chosen:
  for +w: Ir.Which
  for es: List<&2, Ir.Entry>
  {Ir.Approved.run(Ir.approve(w, es)) == S.chosen(w, S.waiting(es)) : List<&2, Nat>}

# LAW irreversible_approve_leaves_rest (R6): after an approval exactly the
# waiting ids it did not select still wait, in order
# EXCLUDES: an approval that clears the whole ledger, or leaves a run effect
#   waiting (to run again)
law irreversible_approve_leaves_rest:
  for +w: Ir.Which
  for es: List<&2, Ir.Entry>
  {S.waiting(Ir.Approved.ledger(Ir.approve(w, es))) == S.left(w, S.waiting(es)) : List<&2, Nat>}

# LAW irreversible_approve_keeps_ids (R6): an approval drops and adds no
# entry
# EXCLUDES: a ledger that forgets an approved effect (its id then reusable)
law irreversible_approve_keeps_ids:
  for +w: Ir.Which
  for es: List<&2, Ir.Entry>
  {ir_ids(Ir.Approved.ledger(Ir.approve(w, es))) == ir_ids(es) : List<&2, Nat>}

# LAW irreversible_approve_once (R6): the same approval twice runs nothing
# the second time
# EXCLUDES: a repeated `/approve` (a second client, a resent command) that
#   runs a push again
law irreversible_approve_once:
  for +w: Ir.Which
  for es: List<&2, Ir.Entry>
  {Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve(w, es)))) == [] : List<&2, Nat>}

# Laws: a ledger's history (R7)
# -----------------------------

# LAW irreversible_silence_runs_nothing (R7): a history with no approval runs
# nothing, whatever it records and whatever the ledger held
# EXCLUDES: a timeout, a detach or a run's end read as a yes; an effect run
#   because it waited long enough
law irreversible_silence_runs_nothing:
  for ops: List<&2, Ir.Op>
  for +es: List<&2, Ir.Entry>
  for quiet: {S.no_approval(ops) == True{} : Bool}
  {Ir.Hist.ran(Ir.run(ops, es)) == [] : List<&2, Nat>}

# LAW irreversible_runs_only_approved (R7): every id a history runs is one
# an approval of that history selects
# EXCLUDES: an approval of one effect that runs another, a record that runs
#   its effect
law irreversible_runs_only_approved:
  for +ops: List<&2, Ir.Op>
  for +es: List<&2, Ir.Entry>
  {S.all_approved(ops, Ir.Hist.ran(Ir.run(ops, es))) == True{} : Bool}

# Laws: the entry (R8)
# --------------------

# LAW irreversible_entry_route (R8): the entry answers a spelled route
# request whose lists fit the wire with the spec's route, spelled
# EXCLUDES: grant and codes read in the other order, a mode tag misread
law irreversible_entry_route:
  for +auto: List<&2, Nat>
  for +effs: List<&2, Nat>
  for +m: Ir.Mode
  for fa: {ir_ns_fit(auto) == True{} : Bool}
  for fe: {ir_ns_fit(effs) == True{} : Bool}
  {FIr.ffi(ir_w_route(auto, effs, m, "")) == ir_w_route_reply(S.route(effs, auto, m)) : String}

# LAW irreversible_entry_settle (R8): the entry answers a spelled settle
# request with the spec's fate, spelled
# EXCLUDES: the answer tags misread (no answer read as a yes)
law irreversible_entry_settle:
  for s: Ir.Said
  {FIr.ffi(ir_w_settle(s, "")) == ir_w_fate_reply(S.settle(s)) : String}

# LAW irreversible_entry_hold (R8): the entry answers a spelled hold request
# that fits the wire with the model's ledger, spelled
# EXCLUDES: a ledger reply in another order or with states swapped
law irreversible_entry_hold:
  for +es: List<&2, Ir.Entry>
  for +i: Nat
  for fl: {ir_ns_fit(ir_flat(es)) == True{} : Bool}
  for fi: {LawsBase.wire_fits(i) == True{} : Bool}
  {FIr.ffi(ir_w_hold(es, i, "")) == ir_w_ledger(Ir.hold(es, i), "") : String}

# LAW irreversible_entry_approve (R8): the entry answers a spelled approve
# request that fits the wire with the model's ids to run, then its ledger,
# spelled
# EXCLUDES: the selection read as all when it names ids, the run ids and
#   the ledger swapped
law irreversible_entry_approve:
  for +w: Ir.Which
  for +es: List<&2, Ir.Entry>
  for fw: {ir_which_fit(w) == True{} : Bool}
  for fl: {ir_ns_fit(ir_flat(es)) == True{} : Bool}
  {FIr.ffi(ir_w_approve(w, es, "")) == ir_w_ns(Ir.Approved.run(Ir.approve(w, es)), ir_w_ledger(Ir.Approved.ledger(Ir.approve(w, es)), "")) : String}

# LAW irreversible_entry_waiting (R8): the entry answers a spelled waiting
# request that fits the wire with the spec's end-of-run list, spelled
# EXCLUDES: a list of every entry, done ones included
law irreversible_entry_waiting:
  for +es: List<&2, Ir.Entry>
  for fl: {ir_ns_fit(ir_flat(es)) == True{} : Bool}
  {FIr.ffi(ir_w_waiting(es, "")) == ir_w_ns(S.waiting(es), "") : String}

# LAW irreversible_entry_refuses (R8): every error the entry answers is
# exactly `E irreversible: malformed request`
# EXCLUDES: an error text that varies with the input, a second error form
law irreversible_entry_refuses:
  for +s: String
  for bad: {LawsBase.wire_opens_err(FIr.ffi(s)) == True{} : Bool}
  {FIr.ffi(s) == Wr.err("irreversible: malformed request") : String}

# LAW irreversible_entry_example (R1-R8): anchor: the wire strings Rust
# sends and the replies it reads; a push (net) waits under the shipped
# grant, an edit runs, watch asks; no answer waits; a ledger of w1 waiting
# and w2 done: `all` runs only 1; malformed requests
# EXCLUDES: a field order other than Rust's, another reply spelling
law irreversible_entry_example:
  {[FIr.ffi("0,3,0,1,2,1,3,0,"), FIr.ffi("0,3,0,1,2,1,1,0,"), FIr.ffi("0,3,0,1,2,1,3,1,"), FIr.ffi("1,2,"), FIr.ffi("3,0,4,1,0,2,1,"), FIr.ffi("4,4,1,0,2,1,"), FIr.ffi("2,0,7,"), FIr.ffi("3,1,1,2,4,1,0,2,0,")] == ["1,", "0,", "2,", "2,", "1,1,4,1,1,2,1,", "1,1,", "2,7,0,", "1,2,4,1,0,2,1,"] : List<&2, String>} &
  {[FIr.ffi("5,"), FIr.ffi("1,3,"), FIr.ffi("0,3,0,1,2,1,3,2,"), FIr.ffi("4,2,1,2,"), FIr.ffi("4,1,5,"), FIr.ffi("1,2,x")] == [Wr.err("irreversible: malformed request"), Wr.err("irreversible: malformed request"), Wr.err("irreversible: malformed request"), Wr.err("irreversible: malformed request"), Wr.err("irreversible: malformed request"), Wr.err("irreversible: malformed request")] : List<&2, String>}
