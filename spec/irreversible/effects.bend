# irreversible (spec): what the laws of area irreversible
# (laws/irreversible.bend) speak of, over the types of
# bend/irreversible.bend: which effects run unasked, which wait for the
# principal's approval, and what an approval runs. A spec: the shipped
# code never calls it.
#
# S1 (I1): an effect is in the grant exactly when each of its codes is a
#     code of the vocabulary (0 read, 1 write, 2 exec, 3 net, 4 outside,
#     5 protected, 6 secret) that the grant holds; an effect of unknown
#     kind (a code past 6) is in no grant. Reversible under the shipped
#     grant: read, write (inside the workspace), exec.
# S2 (I2, I3, I9): an effect in the grant runs; any other waits in the
#     default mode and is asked in watch mode.
# S3 (I5): a yes runs it, a no refuses it, no answer leaves it waiting.
# S4 (I6): the end-of-run list is the ids of the waiting entries, in the
#     order they were recorded.
# S5 (I4): an approval runs exactly the waiting entries it selects, in
#     ledger order, and leaves exactly the others waiting.
# S6 (I4, I5): over any history, an id runs only when an approval of that
#     history selects it.

import Base
import ../../bend/irreversible.bend as Ir

# c is one of the codes 0 to n-1
def below(n: Nat, c: Nat) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+m:
      match c:
        case 0n:
          True{}
        case 1n+d:
          below(m, d)

def has(xs: List<&2, Nat>, +x: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case y <> rest:
      Bool.or(Nat.is_eq(y, x), has(rest, x))

# S1: every code of effs is a known code the grant holds
def in_grant(effs: List<&2, Nat>, +auto: List<&2, Nat>) -> Bool:
  match effs:
    case Nil{}:
      True{}
    case +c <> rest:
      Bool.and(Bool.and(below(7n, c), has(auto, c)), in_grant(rest, auto))

# S1: every code of effs is read, write or exec
def reversible(effs: List<&2, Nat>) -> Bool:
  match effs:
    case Nil{}:
      True{}
    case c <> rest:
      Bool.and(below(3n, c), reversible(rest))

def route.of(granted: Bool, m: Ir.Mode) -> Ir.Route:
  match granted:
    case True{}:
      Ir.Runs{}
    case False{}:
      match m:
        case Ir.End{}:
          Ir.Waits{}
        case Ir.Watch{}:
          Ir.Asks{}

# S2
def route(effs: List<&2, Nat>, +auto: List<&2, Nat>, m: Ir.Mode) -> Ir.Route:
  route.of(in_grant(effs, auto), m)

# S3
def settle(s: Ir.Said) -> Ir.Fate:
  match s:
    case Ir.Yes{}:
      Ir.Ran{}
    case Ir.No{}:
      Ir.Refused{}
    case Ir.Silent{}:
      Ir.Held{}

def is_waiting(s: Ir.St) -> Bool:
  match s:
    case Ir.Waiting{}:
      True{}
    case Ir.Taken{}:
      False{}

# x in front of xs when b
def keep(b: Bool, x: Nat, xs: List<&2, Nat>) -> List<&2, Nat>:
  match b:
    case True{}:
      x <> xs
    case False{}:
      xs

# S4: the ids of the waiting entries, in order
def waiting(es: List<&2, Ir.Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      match e:
        case Ir.Entry{i, s}:
          keep(is_waiting(s), i, waiting(rest))

# S5: an approval selects i
def selects(w: Ir.Which, +i: Nat) -> Bool:
  match w:
    case Ir.All{}:
      True{}
    case Ir.Ids{ks}:
      has(ks, i)

# S5: the ids of xs the approval selects, in order
def chosen(+w: Ir.Which, xs: List<&2, Nat>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> rest:
      keep(selects(w, x), x, chosen(w, rest))

# S5: the ids of xs the approval leaves, in order
def left(+w: Ir.Which, xs: List<&2, Nat>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> rest:
      keep(Bool.not(selects(w, x)), x, left(w, rest))

# S6: some approval among ops selects i
def approved(ops: List<&2, Ir.Op>, +i: Nat) -> Bool:
  match ops:
    case Nil{}:
      False{}
    case op <> rest:
      match op:
        case Ir.Hold{j}:
          approved(rest, i)
        case Ir.Approve{w}:
          Bool.or(selects(w, i), approved(rest, i))

# S6: an approval among ops selects every id of xs
def all_approved(+ops: List<&2, Ir.Op>, xs: List<&2, Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case +x <> rest:
      Bool.and(approved(ops, x), all_approved(ops, rest))

# S6: ops hold no approval
def no_approval(ops: List<&2, Ir.Op>) -> Bool:
  match ops:
    case Nil{}:
      True{}
    case op <> rest:
      match op:
        case Ir.Hold{j}:
          no_approval(rest)
        case Ir.Approve{w}:
          False{}
