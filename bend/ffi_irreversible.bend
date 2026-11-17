# ffi_irreversible: the entry of the compiled Bend core for effects
# outside the principal's grant and the ledger of the ones that wait
# (bend/irreversible.bend; Rust: eukhe-bend rules/irreversible.rs, asked by
# the gates of eukhe-core and the session's `eukhe.effects` ledger).
# Grammar: crates/eukhe-bend/WIRE.md. A request is a constructor:
#
#   tag 0  route    the grant (List<Nat>), the effect codes (List<Nat>),
#                   the mode (0 end, 1 watch)
#                   -> 0 runs | 1 waits | 2 asks
#   tag 1  settle   how a question ended (0 yes, 1 no, 2 no answer)
#                   -> 0 runs | 1 refused | 2 waits
#   tag 2  hold     the ledger, an id -> the ledger
#   tag 3  approve  the selection (0 all | 1 then the ids, List<Nat>), the
#                   ledger -> the ids to run (List<Nat>), then the ledger
#   tag 4  waiting  the ledger -> the waiting ids (List<Nat>)
#
# A ledger is one List<Nat> of pairs: each entry's id, then its state (0
# waiting, 1 done). Malformed input (an unknown tag, a state, mode or
# answer past its last tag, input left after the request) answers
# `E irreversible: malformed request`, never a default. Pure: no IO, no
# @unsafe.

import Base
import ./wire.bend as W
import ./irreversible.bend as Ir

# A request of the entry
type Req is Data:
  QRoute{auto: List<&2, Nat>, effs: List<&2, Nat>, mode: Ir.Mode}
  QSettle{said: Ir.Said}
  QHold{ledger: List<&2, Ir.Entry>, id: Nat}
  QApprove{which: Ir.Which, ledger: List<&2, Ir.Entry>}
  QWaiting{ledger: List<&2, Ir.Entry>}

# Values
# ------

def get_ns(s: String) -> W.Dec<List<&2, Nat>>:
  W.get_list(~Nat, ~W.get_nat, s)

def put_ns(xs: List<&2, Nat>, rest: String) -> String:
  W.put_list(~Nat, ~W.put_nat, xs, rest)

def st.of(n: Nat) -> Maybe<&2, Ir.St>:
  match n:
    case 0n:
      Some{Ir.Waiting{}}
    case 1n+a:
      match a:
        case 0n:
          Some{Ir.Taken{}}
        case 1n+b:
          None{}

def pairs.cons(+i: Nat, m: Maybe<&2, Ir.St>, rest: Maybe<&2, List<&2, Ir.Entry>>) -> Maybe<&2, List<&2, Ir.Entry>>:
  match m:
    case None{}:
      None{}
    case Some{st}:
      match rest:
        case None{}:
          None{}
        case Some{es}:
          Some{Ir.Entry{i, st} <> es}

# the entries the pairs of ns spell: id, state, id, state, ...; an odd
# count or a state past 1 spells none
def pairs(ns: List<&2, Nat>) -> Maybe<&2, List<&2, Ir.Entry>>:
  match ns:
    case Nil{}:
      Some{Nil{}}
    case +i <> rest:
      match rest:
        case Nil{}:
          None{}
        case t <> more:
          pairs.cons(i, st.of(t), pairs(more))

def st.tag(st: Ir.St) -> Nat:
  match st:
    case Ir.Waiting{}:
      0n
    case Ir.Taken{}:
      1n

# the pairs of the entries
def flat(es: List<&2, Ir.Entry>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case e <> rest:
      match e:
        case Ir.Entry{i, st}:
          i <> (st.tag(st) <> flat(rest))

def get_ledger.of(m: Maybe<&2, List<&2, Ir.Entry>>, s: String) -> W.Dec<List<&2, Ir.Entry>>:
  match m:
    case None{}:
      W.Bad{}
    case Some{es}:
      W.Got{es, s}

def get_ledger.go(r: W.Dec<List<&2, Nat>>) -> W.Dec<List<&2, Ir.Entry>>:
  match r:
    case W.Got{ns, s}:
      get_ledger.of(pairs(ns), s)
    case W.Bad{}:
      W.Bad{}

def get_ledger(s: String) -> W.Dec<List<&2, Ir.Entry>>:
  get_ledger.go(get_ns(s))

def put_ledger(es: List<&2, Ir.Entry>, rest: String) -> String:
  put_ns(flat(es), rest)

# Requests
# --------

def mode.of(n: Nat) -> Maybe<&2, Ir.Mode>:
  match n:
    case 0n:
      Some{Ir.End{}}
    case 1n+a:
      match a:
        case 0n:
          Some{Ir.Watch{}}
        case 1n+b:
          None{}

def get_route.mode(auto: List<&2, Nat>, effs: List<&2, Nat>, m: Maybe<&2, Ir.Mode>, s: String) -> W.Dec<Req>:
  match m:
    case None{}:
      W.Bad{}
    case Some{md}:
      W.Got{QRoute{auto, effs, md}, s}

def get_route.tag(auto: List<&2, Nat>, effs: List<&2, Nat>, r: W.Dec<Nat>) -> W.Dec<Req>:
  match r:
    case W.Got{n, s}:
      get_route.mode(auto, effs, mode.of(n), s)
    case W.Bad{}:
      W.Bad{}

def get_route.effs(auto: List<&2, Nat>, r: W.Dec<List<&2, Nat>>) -> W.Dec<Req>:
  match r:
    case W.Got{effs, s}:
      get_route.tag(auto, effs, W.get_nat(s))
    case W.Bad{}:
      W.Bad{}

def get_route(r: W.Dec<List<&2, Nat>>) -> W.Dec<Req>:
  match r:
    case W.Got{auto, s}:
      get_route.effs(auto, get_ns(s))
    case W.Bad{}:
      W.Bad{}

def said.of(n: Nat) -> Maybe<&2, Ir.Said>:
  match n:
    case 0n:
      Some{Ir.Yes{}}
    case 1n+a:
      match a:
        case 0n:
          Some{Ir.No{}}
        case 1n+b:
          match b:
            case 0n:
              Some{Ir.Silent{}}
            case 1n+c:
              None{}

def get_settle.said(m: Maybe<&2, Ir.Said>, s: String) -> W.Dec<Req>:
  match m:
    case None{}:
      W.Bad{}
    case Some{sd}:
      W.Got{QSettle{sd}, s}

def get_settle(r: W.Dec<Nat>) -> W.Dec<Req>:
  match r:
    case W.Got{n, s}:
      get_settle.said(said.of(n), s)
    case W.Bad{}:
      W.Bad{}

def get_hold.id(es: List<&2, Ir.Entry>, r: W.Dec<Nat>) -> W.Dec<Req>:
  match r:
    case W.Got{i, s}:
      W.Got{QHold{es, i}, s}
    case W.Bad{}:
      W.Bad{}

def get_hold(r: W.Dec<List<&2, Ir.Entry>>) -> W.Dec<Req>:
  match r:
    case W.Got{es, s}:
      get_hold.id(es, W.get_nat(s))
    case W.Bad{}:
      W.Bad{}

def get_which.ids(r: W.Dec<List<&2, Nat>>) -> W.Dec<Ir.Which>:
  match r:
    case W.Got{ks, s}:
      W.Got{Ir.Ids{ks}, s}
    case W.Bad{}:
      W.Bad{}

def get_which.tag(n: Nat, s: String) -> W.Dec<Ir.Which>:
  match n:
    case 0n:
      W.Got{Ir.All{}, s}
    case 1n+a:
      match a:
        case 0n:
          get_which.ids(get_ns(s))
        case 1n+b:
          W.Bad{}

def get_which.go(r: W.Dec<Nat>) -> W.Dec<Ir.Which>:
  match r:
    case W.Got{n, t}:
      get_which.tag(n, t)
    case W.Bad{}:
      W.Bad{}

# a selection: 0 all, or 1 then the ids
def get_which(s: String) -> W.Dec<Ir.Which>:
  get_which.go(W.get_nat(s))

def get_approve.ledger(w: Ir.Which, r: W.Dec<List<&2, Ir.Entry>>) -> W.Dec<Req>:
  match r:
    case W.Got{es, s}:
      W.Got{QApprove{w, es}, s}
    case W.Bad{}:
      W.Bad{}

def get_approve(r: W.Dec<Ir.Which>) -> W.Dec<Req>:
  match r:
    case W.Got{w, s}:
      get_approve.ledger(w, get_ledger(s))
    case W.Bad{}:
      W.Bad{}

def get_waiting(r: W.Dec<List<&2, Ir.Entry>>) -> W.Dec<Req>:
  match r:
    case W.Got{es, s}:
      W.Got{QWaiting{es}, s}
    case W.Bad{}:
      W.Bad{}

def get_req.tag(t: Nat, s: String) -> W.Dec<Req>:
  match t:
    case 0n:
      get_route(get_ns(s))
    case 1n:
      get_settle(W.get_nat(s))
    case 2n:
      get_hold(get_ledger(s))
    case 3n:
      get_approve(get_which(s))
    case 4n:
      get_waiting(get_ledger(s))
    case 5n+u:
      W.Bad{}

def get_req.go(r: W.Dec<Nat>) -> W.Dec<Req>:
  match r:
    case W.Got{t, s}:
      get_req.tag(t, s)
    case W.Bad{}:
      W.Bad{}

# the request at the start of s
def get_req(s: String) -> W.Dec<Req>:
  get_req.go(W.get_nat(s))

# Replies
# -------

def put_route(r: Ir.Route) -> String:
  match r:
    case Ir.Runs{}:
      W.put_nat(0n, "")
    case Ir.Waits{}:
      W.put_nat(1n, "")
    case Ir.Asks{}:
      W.put_nat(2n, "")

def put_fate(f: Ir.Fate) -> String:
  match f:
    case Ir.Ran{}:
      W.put_nat(0n, "")
    case Ir.Refused{}:
      W.put_nat(1n, "")
    case Ir.Held{}:
      W.put_nat(2n, "")

def put_approved(a: Ir.Approved) -> String:
  match a:
    case Ir.Approved{r, l}:
      put_ns(r, put_ledger(l, ""))

# the model's decision on a request, encoded
def answer(q: Req) -> String:
  match q:
    case QRoute{+auto, +effs, m}:
      put_route(Ir.route(effs, auto, m))
    case QSettle{sd}:
      put_fate(Ir.settle(sd))
    case QHold{es, +i}:
      put_ledger(Ir.hold(es, i), "")
    case QApprove{+w, es}:
      put_approved(Ir.approve(w, es))
    case QWaiting{es}:
      put_ns(Ir.waiting(es), "")

def malformed() -> String:
  W.err("irreversible: malformed request")

def ffi.done(q: Req, end: Bool) -> String:
  match end:
    case True{}:
      answer(q)
    case False{}:
      malformed()

def ffi.got(r: W.Dec<Req>) -> String:
  match r:
    case W.Got{q, +s}:
      ffi.done(q, W.end(s))
    case W.Bad{}:
      malformed()

# the entry: decode the request, answer the model's decision
def ffi(input: String) -> String:
  ffi.got(get_req(input))
