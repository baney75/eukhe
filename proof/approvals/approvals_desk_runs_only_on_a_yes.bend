# eukhe: the proof of LAW approvals_desk_runs_only_on_a_yes
# PROOF: a case per event. An ask shows, relays or ends unanswered: no
# wake runs. An answer wakes the question it takes with the answer's own
# fate: it runs only on a yes. A detach that leaves no client relays or
# ends unanswered each question it moves (by induction on the questions);
# one that leaves a client does nothing. A cancel wakes nothing. The
# parent's answer wakes the question `take` found, whose key is the
# answer's (ry.take_key, by induction on the questions), with the
# parent's fate.

import Base
import ../../bend/approvals.bend as Am
import ../../laws/approvals/desk.bend as ApprovalsDesk
import ./base/desk.bend as LAD

# An ask
# ------

def ry.ask(+n: Nat, +id: String, +es: List<&2, Nat>, +qs: List<&2, Am.Q>, h: Am.Here, p: Bool) -> {ApprovalsDesk.dy_yes_only(Am.EAsk{id, es, h, p}, ApprovalsDesk.ds_acts(Am.ask.with(n, id, es, qs, Am.next(es, h, p)))) == True{} : Bool}:
  match h:
    case Am.Nobody{}:
      match p:
        case True{}:
          {==}
        case False{}:
          {==}
    case Am.Clients{}:
      match p:
        case True{}:
          {==}
        case False{}:
          {==}

# An answer
# ---------

def ry.answer(+n: Nat, +qs: List<&2, Am.Q>, +id: String, y: Bool, r: Am.QF) -> {ApprovalsDesk.dy_yes_only(Am.EAnswer{id, y}, ApprovalsDesk.ds_acts(Am.answer.with(n, qs, y, r))) == True{} : Bool}:
  match y:
    case True{}:
      match r:
        case Am.QFound{q, rest}:
          match q:
            case Am.Q{k, i, es, w}:
              {==}
        case Am.QNone{}:
          {==}
    case False{}:
      match r:
        case Am.QFound{q, rest}:
          match q:
            case Am.Q{k, i, es, w}:
              {==}
        case Am.QNone{}:
          {==}

# A detach
# --------

def ry.q(w: Am.Wait, +k: Nat, +i: String, +es: List<&2, Nat>, p: Bool, r: List<&2, Am.Q> & List<&2, Am.Act>, ih: {ApprovalsDesk.dy_yes_only(Am.EDetach{Am.Nobody{}, p}, LAD.ad.as(r)) == True{} : Bool}) -> {ApprovalsDesk.dy_yes_only(Am.EDetach{Am.Nobody{}, p}, LAD.ad.as(Am.reroute.q(w, k, i, es, Am.next(es, Am.Nobody{}, p), r))) == True{} : Bool}:
  match w:
    case Am.OnParent{}:
      (qs, as) = r
      ih
    case Am.OnClients{}:
      match p:
        case True{}:
          (qs, as) = r
          ih
        case False{}:
          (qs, as) = r
          ih

def ry.rr(qs: List<&2, Am.Q>, +p: Bool) -> {ApprovalsDesk.dy_yes_only(Am.EDetach{Am.Nobody{}, p}, LAD.ad.as(Am.reroute(qs, Am.Nobody{}, p))) == True{} : Bool}:
  match qs:
    case Nil{}:
      {==}
    case q <> +rest:
      match q:
        case Am.Q{k, i, es, w}:
          ry.q(w, k, i, es, p, Am.reroute(rest, Am.Nobody{}, p), ry.rr(rest, p))

def ry.put(+n: Nat, +p: Bool, r: List<&2, Am.Q> & List<&2, Am.Act>, ih: {ApprovalsDesk.dy_yes_only(Am.EDetach{Am.Nobody{}, p}, LAD.ad.as(r)) == True{} : Bool}) -> {ApprovalsDesk.dy_yes_only(Am.EDetach{Am.Nobody{}, p}, ApprovalsDesk.ds_acts(Am.detach.put(n, r))) == True{} : Bool}:
  (qs, as) = r
  ih

def ry.detach(+n: Nat, +qs: List<&2, Am.Q>, h: Am.Here, +p: Bool) -> {ApprovalsDesk.dy_yes_only(Am.EDetach{h, p}, ApprovalsDesk.ds_acts(Am.detach.with(n, qs, h, p))) == True{} : Bool}:
  match h:
    case Am.Clients{}:
      {==}
    case Am.Nobody{}:
      ry.put(n, p, Am.reroute(qs, Am.Nobody{}, p), ry.rr(qs, p))

# A cancel
# --------

def ry.cancel(+n: Nat, +qs: List<&2, Am.Q>, +key: Nat, r: Am.QF) -> {ApprovalsDesk.dy_yes_only(Am.ECancel{key}, ApprovalsDesk.ds_acts(Am.cancel.with(n, qs, r))) == True{} : Bool}:
  match r:
    case Am.QFound{q, rest}:
      match q:
        case Am.Q{k, i, es, w}:
          match w:
            case Am.OnClients{}:
              {==}
            case Am.OnParent{}:
              {==}
    case Am.QNone{}:
      {==}

# The parent's answer
# -------------------

# the question r found, if any, has the key key
def ry.found_key(r: Am.QF, +key: Nat) -> Bool:
  match r:
    case Am.QFound{q, rest}:
      match q:
        case Am.Q{k, i, es, w}:
          Nat.is_eq(k, key)
    case Am.QNone{}:
      True{}

def ry.pick(b: Bool, +k: Nat, +i: String, +es: List<&2, Nat>, +w: Am.Wait, +key: Nat, eb: {Nat.is_eq(k, key) == b : Bool}, +rest: List<&2, Am.Q>, r: Am.QF, ih: {ry.found_key(r, key) == True{} : Bool}) -> {ry.found_key(Am.pick.q(b, Am.Q{k, i, es, w}, rest, r), key) == True{} : Bool}:
  match b:
    case True{}:
      eb
    case False{}:
      match r:
        case Am.QFound{q2, r2}:
          match q2:
            case Am.Q{k2, i2, es2, w2}:
              ih
        case Am.QNone{}:
          {==}

# take finds a question with the key it was asked for, or none
def ry.take_key(qs: List<&2, Am.Q>, +key: Nat) -> {ry.found_key(Am.take(qs, key), key) == True{} : Bool}:
  match qs:
    case Nil{}:
      {==}
    case q <> +rest:
      match q:
        case Am.Q{+k, +i, +es, +w}:
          ry.pick(Nat.is_eq(k, key), k, i, es, w, key, {==}, rest, Am.take(rest, key), ry.take_key(rest, key))

# a wake with the parent's fate f, on a question whose key is the answer's
# (b: the keys match), passes the yes check
def ry.back_runs(b: Bool, f: Am.Fate, eb: {b == True{} : Bool}) -> {Bool.and(Bool.or(Bool.not(ApprovalsDesk.dy_runs(f)), Bool.and(b, ApprovalsDesk.dy_runs(f))), True{}) == True{} : Bool}:
  match b:
    case True{}:
      match f:
        case Am.Runs{}:
          {==}
        case Am.Denied{}:
          {==}
        case Am.Unheard{}:
          {==}
    case False{}:
      LAD.ad.absurd({Bool.and(Bool.or(Bool.not(ApprovalsDesk.dy_runs(f)), Bool.and(False{}, ApprovalsDesk.dy_runs(f))), True{}) == True{} : Bool}, eb)

def ry.back(+n: Nat, +qs: List<&2, Am.Q>, +key: Nat, +f: Am.Fate, r: Am.QF, hk: {ry.found_key(r, key) == True{} : Bool}) -> {ApprovalsDesk.dy_yes_only(Am.EBack{key, f}, ApprovalsDesk.ds_acts(Am.back.with(n, qs, f, r))) == True{} : Bool}:
  match r:
    case Am.QFound{q, rest}:
      match q:
        case Am.Q{+k, i, es, w}:
          match w:
            case Am.OnParent{}:
              ry.back_runs(Nat.is_eq(k, key), f, hk)
            case Am.OnClients{}:
              {==}
    case Am.QNone{}:
      {==}

def ApprovalsDesk.approvals_desk_runs_only_on_a_yes(d, e):
  match d:
    case Am.D{+n, +qs}:
      match e:
        case Am.EAsk{+id, +es, h, p}:
          ry.ask(n, id, es, qs, h, p)
        case Am.EAnswer{+id, y}:
          ry.answer(n, qs, id, y, Am.find(qs, id))
        case Am.EDetach{h, +p}:
          ry.detach(n, qs, h, p)
        case Am.ECancel{+key}:
          ry.cancel(n, qs, key, Am.take(qs, key))
        case Am.EBack{+key, +f}:
          ry.back(n, qs, key, f, Am.take(qs, key), ry.take_key(qs, key))
