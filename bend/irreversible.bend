# irreversible: the model of what becomes of an effect outside the
# principal's grant, and of the ledger of the effects that wait for the
# principal's approval (spec/README.md I1-I6, I9; spec/irreversible/
# effects.bend restates it). Rust: crates/eukhe-bend/src/rules/
# irreversible.rs through bend/ffi_irreversible.bend, asked by the gates of
# eukhe-core (a `bash` step, a document tool's access, a `python` cell, an
# MCP call) and by the session's `eukhe.effects` ledger.
#
# An effect is its effect codes (bend/tools.bend: 0 read, 1 write, 2 exec,
# 3 net, 4 outside, 5 protected, 6 secret; a code past 6 is of unknown
# kind). It runs unasked exactly when every code is in the grant (the auto
# set, `Tl.within`; a code past 6 is in none). Any other effect, in the
# default mode, is not run and no one is asked: it is recorded as waiting.
# In watch mode it is asked; a yes runs it, a no refuses it, and no answer
# records it as waiting. A waiting effect runs only when an approval
# selects it: all of them, or the ones it names; each runs once.

import Base
import ./tools.bend as Tl

# Whether the principal watches the run
type Mode is Data:
  End{}     # the default: nothing is asked; what needs approval waits
  Watch{}   # every effect outside the grant is asked

# What becomes of an effect when it is submitted
type Route is Data:
  Runs{}    # in the grant: it runs now, unasked
  Waits{}   # recorded as waiting for an approval
  Asks{}    # asked of the principal (watch mode)

# How a question of watch mode ended
type Said is Data:
  Yes{}
  No{}
  Silent{}  # no answer: every client left, the deadline, a stop

# What the end of a question does to its effect
type Fate is Data:
  Ran{}     # it runs
  Refused{} # it does not run and does not wait
  Held{}    # it waits for an approval

# The state of a ledger entry
type St is Data:
  Waiting{}
  Taken{}   # an approval took it: it ran, or runs

# One entry of the ledger: the effect's id and its state
type Entry is Data:
  Entry{id: Nat, st: St}

# What an approval selects
type Which is Data:
  All{}
  Ids{ids: List<&2, Nat>}

# What an approval does: the ids to run, in ledger order, and the ledger
# after it
type Approved is Data:
  Approved{run: List<&2, Nat>, ledger: List<&2, Entry>}

def Approved.run(a: Approved) -> List<&2, Nat>:
  match a:
    case Approved{r, l}:
      r

def Approved.ledger(a: Approved) -> List<&2, Entry>:
  match a:
    case Approved{r, l}:
      l

# 1. Route and fate
# -----------------

def route.out(m: Mode) -> Route:
  match m:
    case End{}:
      Waits{}
    case Watch{}:
      Asks{}

def route.of(granted: Bool, m: Mode) -> Route:
  match granted:
    case True{}:
      Runs{}
    case False{}:
      route.out(m)

# what becomes of an effect with codes effs under the grant auto in mode m
def route(+effs: List<&2, Nat>, +auto: List<&2, Nat>, m: Mode) -> Route:
  route.of(Tl.within(effs, auto), m)

# what the end of a watch-mode question does
def settle(s: Said) -> Fate:
  match s:
    case Yes{}:
      Ran{}
    case No{}:
      Refused{}
    case Silent{}:
      Held{}

# 2. The ledger
# -------------

def Entry.id(e: Entry) -> Nat:
  match e:
    case Entry{i, s}:
      i

def ids(es: List<&2, Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      Entry.id(e) <> ids(rest)

def waiting.add(+i: Nat, s: St, acc: List<&2, Nat>) -> List<&2, Nat>:
  match s:
    case Waiting{}:
      i <> acc
    case Taken{}:
      acc

# the waiting ids, in the order they were recorded: the end-of-run list
def waiting(es: List<&2, Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      match e:
        case Entry{+i, s}:
          waiting.add(i, s, waiting(rest))

def hold.go(known: Bool, es: List<&2, Entry>, +i: Nat) -> List<&2, Entry>:
  match known:
    case True{}:
      es
    case False{}:
      List.append(&2, Entry, es, [Entry{i, Waiting{}}])

# record effect i as waiting, last; an id the ledger already holds changes
# nothing
def hold(+es: List<&2, Entry>, +i: Nat) -> List<&2, Entry>:
  hold.go(Tl.elem(ids(es), i), es, i)

# the approval w selects id i
def picks(w: Which, +i: Nat) -> Bool:
  match w:
    case All{}:
      True{}
    case Ids{ks}:
      Tl.elem(ks, i)

def approve.cons(+i: Nat, s: St, take: Bool, rest: Approved) -> Approved:
  match take:
    case True{}:
      match rest:
        case Approved{r, l}:
          Approved{i <> r, Entry{i, Taken{}} <> l}
    case False{}:
      match rest:
        case Approved{r, l}:
          Approved{r, Entry{i, s} <> l}

def approve.take(s: St, w: Which, +i: Nat) -> Bool:
  match s:
    case Waiting{}:
      picks(w, i)
    case Taken{}:
      False{}

# an approval: every waiting entry it selects runs, in ledger order, and is
# done; every other entry stays
def approve(+w: Which, es: List<&2, Entry>) -> Approved:
  match es:
    case Nil{}:
      Approved{Nil{}, Nil{}}
    case e <> rest:
      match e:
        case Entry{+i, +s}:
          approve.cons(i, s, approve.take(s, w, i), approve(w, rest))

# 3. A ledger's history
# ---------------------

# What happens to a ledger: an effect is recorded waiting, or an approval
# comes
type Op is Data:
  Hold{id: Nat}
  Approve{which: Which}

# What a history did: the ids it ran, in order, and the ledger it left
type Hist is Data:
  Hist{ran: List<&2, Nat>, ledger: List<&2, Entry>}

def Hist.ran(h: Hist) -> List<&2, Nat>:
  match h:
    case Hist{r, l}:
      r

def Hist.ledger(h: Hist) -> List<&2, Entry>:
  match h:
    case Hist{r, l}:
      l

def hist.after(first: List<&2, Nat>, h: Hist) -> Hist:
  match h:
    case Hist{r, l}:
      Hist{List.append(&2, Nat, first, r), l}

# the ops in order from the ledger es
def run(ops: List<&2, Op>, es: List<&2, Entry>) -> Hist:
  match ops:
    case Nil{}:
      Hist{Nil{}, es}
    case op <> rest:
      match op:
        case Hold{i}:
          run(rest, hold(es, i))
        case Approve{w}:
          +a = approve(w, es)
          hist.after(Approved.run(a), run(rest, Approved.ledger(a)))
