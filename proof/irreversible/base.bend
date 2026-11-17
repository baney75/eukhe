# eukhe: lemmas shared by the proofs of area irreversible

import Base
import ../../bend/irreversible.bend as Ir
import ../../bend/tools.bend as Tl
import ../../bend/approvals.bend as Am
import ../../spec/irreversible/effects.bend as S
import ../../laws/irreversible.bend as Irreversible
import ../../bend/ffi_irreversible.bend as FIr
import ../../bend/wire.bend as Wr
import ../../laws/wire.bend as Wire
import ../../laws/base.bend as LawsBase

def irb.IfF(b: Bool, G: Type) -> Type:
  match b:
    case True{}:
      G
    case False{}:
      Unit

def irb.IfT(b: Bool, G: Type) -> Type:
  match b:
    case True{}:
      Unit
    case False{}:
      G

# a premise False == True proves anything
def irb.absurd_ft(-G: Type, e: {False{} == True{} : Bool}) -> G:
  %e : irb.IfF(_, G)
  Unit{}

# a premise True == False proves anything
def irb.absurd_tf(-G: Type, e: {True{} == False{} : Bool}) -> G:
  %e : irb.IfT(_, G)
  Unit{}

# The route
# ---------

# the tools' member test is the spec's
def irb.elem_has(xs: List<&2, Nat>, +x: Nat) -> {Tl.elem(xs, x) == S.has(xs, x) : Bool}:
  match xs:
    case Nil{}:
      {==}
    case +y <> +rest:
      %irb.elem_has(rest, x) : {Tl.elem(y <> rest, x) == Bool.or(Nat.is_eq(y, x), _) : Bool}
      {==}

# a grantable code is a code of the vocabulary: below 7
def irb.grantable_below(c: Nat) -> {Tl.grantable(c) == S.below(7n, c) : Bool}:
  match c:
    case 0n:
      {==}
    case 1n+a:
      match a:
        case 0n:
          {==}
        case 1n+b:
          match b:
            case 0n:
              {==}
            case 1n+d:
              match d:
                case 0n:
                  {==}
                case 1n+e:
                  match e:
                    case 0n:
                      {==}
                    case 1n+f:
                      match f:
                        case 0n:
                          {==}
                        case 1n+g:
                          match g:
                            case 0n:
                              {==}
                            case 1n+h:
                              {==}

# the tools' grant test is the spec's
def irb.within_in_grant(effs: List<&2, Nat>, +auto: List<&2, Nat>) -> {Tl.within(effs, auto) == S.in_grant(effs, auto) : Bool}:
  match effs:
    case Nil{}:
      {==}
    case +c <> +rest:
      %irb.grantable_below(c) : {Tl.within(c <> rest, auto) == Bool.and(Bool.and(_, S.has(auto, c)), S.in_grant(rest, auto)) : Bool}
      %irb.elem_has(auto, c) : {Tl.within(c <> rest, auto) == Bool.and(Bool.and(Tl.grantable(c), _), S.in_grant(rest, auto)) : Bool}
      %irb.within_in_grant(rest, auto) : {Tl.within(c <> rest, auto) == Bool.and(Bool.and(Tl.grantable(c), Tl.elem(auto, c)), _) : Bool}
      {==}

def irb.route_of(b: Bool, m: Ir.Mode) -> {Ir.route.of(b, m) == S.route.of(b, m) : Ir.Route}:
  match b:
    case True{}:
      {==}
    case False{}:
      match m:
        case Ir.End{}:
          {==}
        case Ir.Watch{}:
          {==}

# the route runs exactly when the grant holds the effect
def irb.runs_of(b: Bool, m: Ir.Mode) -> {Irreversible.ir_runs(Ir.route.of(b, m)) == b : Bool}:
  match b:
    case True{}:
      {==}
    case False{}:
      match m:
        case Ir.End{}:
          {==}
        case Ir.Watch{}:
          {==}

# under the shipped grant, a code runs unasked exactly when it is read,
# write or exec
def irb.default_code(c: Nat, r: Bool) -> {Bool.and(Bool.and(Tl.grantable(c), Tl.elem(Am.default_auto(), c)), r) == Bool.and(S.below(3n, c), r) : Bool}:
  match c:
    case 0n:
      {==}
    case 1n+a:
      match a:
        case 0n:
          {==}
        case 1n+b:
          match b:
            case 0n:
              {==}
            case 1n+d:
              match d:
                case 0n:
                  {==}
                case 1n+e:
                  match e:
                    case 0n:
                      {==}
                    case 1n+f:
                      match f:
                        case 0n:
                          {==}
                        case 1n+g:
                          match g:
                            case 0n:
                              {==}
                            case 1n+h:
                              {==}

# under the shipped grant, an effect runs unasked exactly when it is
# reversible
def irb.default_within(effs: List<&2, Nat>) -> {Tl.within(effs, Am.default_auto()) == S.reversible(effs) : Bool}:
  match effs:
    case Nil{}:
      {==}
    case +c <> +rest:
      %irb.default_within(rest) : {Tl.within(c <> rest, Am.default_auto()) == Bool.and(S.below(3n, c), _) : Bool}
      irb.default_code(c, Tl.within(rest, Am.default_auto()))

# The ledger
# ----------

def irb.ids(es: List<&2, Ir.Entry>) -> {Ir.ids(es) == Irreversible.ir_ids(es) : List<&2, Nat>}:
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, +st}:
          %irb.ids(rest) : {Ir.ids(Ir.Entry{i, st} <> rest) == i <> _ : List<&2, Nat>}
          {==}

# the waiting list of a ledger with one more waiting entry at its end
def irb.waiting_snoc(es: List<&2, Ir.Entry>, +i: Nat) -> {S.waiting(List.append(&2, Ir.Entry, es, [Ir.Entry{i, Ir.Waiting{}}])) == List.append(&2, Nat, S.waiting(es), [i]) : List<&2, Nat>}:
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+j, st}:
          match st:
            case Ir.Waiting{}:
              %irb.waiting_snoc(rest, i) : {S.waiting(List.append(&2, Ir.Entry, Ir.Entry{j, Ir.Waiting{}} <> rest, [Ir.Entry{i, Ir.Waiting{}}])) == j <> _ : List<&2, Nat>}
              {==}
            case Ir.Taken{}:
              irb.waiting_snoc(rest, i)

# the selection is the spec's
def irb.picks(w: Ir.Which, +i: Nat) -> {Ir.picks(w, i) == S.selects(w, i) : Bool}:
  match w:
    case Ir.All{}:
      {==}
    case Ir.Ids{ks}:
      irb.elem_has(ks, i)

def irb.and_false_r(b: Bool) -> {Bool.and(b, False{}) == False{} : Bool}:
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def irb.or_false_r(b: Bool) -> {Bool.or(b, False{}) == b : Bool}:
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

# Lemmas of the laws
# ------------------

# a code of unknown kind is in no grant
def ir.unknown_out(+c: Nat, +auto: List<&2, Nat>, u: {S.below(7n, c) == False{} : Bool}) -> {Bool.and(Tl.grantable(c), Tl.elem(auto, c)) == False{} : Bool}:
  %Equal.sym(Bool, Tl.grantable(c), False{}, Equal.trans(Bool, Tl.grantable(c), S.below(7n, c), False{}, irb.grantable_below(c), u)) : {Bool.and(_, Tl.elem(auto, c)) == False{} : Bool}
  {==}

# whether the ledger holds id i, as the model asks it, is the spec's
def il.known(+es: List<&2, Ir.Entry>, +i: Nat) -> {Tl.elem(Ir.ids(es), i) == S.has(Irreversible.ir_ids(es), i) : Bool}:
  Equal.trans(Bool, Tl.elem(Ir.ids(es), i), S.has(Ir.ids(es), i), S.has(Irreversible.ir_ids(es), i), irb.elem_has(Ir.ids(es), i), Equal.cong(List<&2, Nat>, Bool, z => S.has(z, i), Ir.ids(es), Irreversible.ir_ids(es), irb.ids(es)))

# Approvals
# ---------

# the ids an entry's step runs
def il.cons_run(+i: Nat, s: Ir.St, b: Bool, a: Ir.Approved) -> {Ir.Approved.run(Ir.approve.cons(i, s, b, a)) == S.keep(b, i, Ir.Approved.run(a)) : List<&2, Nat>}:
  match b:
    case True{}:
      match a:
        case Ir.Approved{r, l}:
          {==}
    case False{}:
      match a:
        case Ir.Approved{r, l}:
          {==}

# the waiting list after a waiting entry's step
def il.wait_led(+i: Nat, b: Bool, a: Ir.Approved) -> {S.waiting(Ir.Approved.ledger(Ir.approve.cons(i, Ir.Waiting{}, b, a))) == S.keep(Bool.not(b), i, S.waiting(Ir.Approved.ledger(a))) : List<&2, Nat>}:
  match b:
    case True{}:
      match a:
        case Ir.Approved{r, l}:
          {==}
    case False{}:
      match a:
        case Ir.Approved{r, l}:
          {==}

# the waiting list after a done entry's step
def il.taken_led(+i: Nat, a: Ir.Approved) -> {S.waiting(Ir.Approved.ledger(Ir.approve.cons(i, Ir.Taken{}, False{}, a))) == S.waiting(Ir.Approved.ledger(a)) : List<&2, Nat>}:
  match a:
    case Ir.Approved{r, l}:
      {==}

# an entry's step keeps its id in front of the rest
def il.cons_ids(+i: Nat, s: Ir.St, b: Bool, a: Ir.Approved) -> {Irreversible.ir_ids(Ir.Approved.ledger(Ir.approve.cons(i, s, b, a))) == i <> Irreversible.ir_ids(Ir.Approved.ledger(a)) : List<&2, Nat>}:
  match b:
    case True{}:
      match a:
        case Ir.Approved{r, l}:
          {==}
    case False{}:
      match a:
        case Ir.Approved{r, l}:
          {==}

# a second approval passes over an entry the first one took
def il.once_taken(+w: Ir.Which, +i: Nat, a: Ir.Approved) -> {Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve.cons(i, Ir.Taken{}, False{}, a)))) == Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(a))) : List<&2, Nat>}:
  match a:
    case Ir.Approved{r, +l}:
      il.cons_run(i, Ir.Taken{}, False{}, Ir.approve(w, l))

# a second approval passes over a waiting entry: the first one took it,
# or it did not select it, and neither does the second
def il.once_waiting(+w: Ir.Which, +i: Nat, b: Bool, eb: {Ir.picks(w, i) == b : Bool}, a: Ir.Approved) -> {Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(Ir.approve.cons(i, Ir.Waiting{}, b, a)))) == Ir.Approved.run(Ir.approve(w, Ir.Approved.ledger(a))) : List<&2, Nat>}:
  match b:
    case True{}:
      match a:
        case Ir.Approved{r, +l}:
          il.cons_run(i, Ir.Taken{}, False{}, Ir.approve(w, l))
    case False{}:
      match a:
        case Ir.Approved{r, +l}:
          %Equal.sym(Bool, Ir.picks(w, i), False{}, eb) : {Ir.Approved.run(Ir.approve.cons(i, Ir.Waiting{}, _, Ir.approve(w, l))) == Ir.Approved.run(Ir.approve(w, l)) : List<&2, Nat>}
          il.cons_run(i, Ir.Waiting{}, False{}, Ir.approve(w, l))

# Lemmas
# ------

def ih.ran_after(f: List<&2, Nat>, h: Ir.Hist) -> {Ir.Hist.ran(Ir.hist.after(f, h)) == List.append(&2, Nat, f, Ir.Hist.ran(h)) : List<&2, Nat>}:
  match h:
    case Ir.Hist{r, l}:
      {==}

def ih.and_assoc(a: Bool, b: Bool, c: Bool) -> {Bool.and(a, Bool.and(b, c)) == Bool.and(Bool.and(a, b), c) : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      {==}

def ih.all_app(+ops: List<&2, Ir.Op>, xs: List<&2, Nat>, +ys: List<&2, Nat>) -> {S.all_approved(ops, List.append(&2, Nat, xs, ys)) == Bool.and(S.all_approved(ops, xs), S.all_approved(ops, ys)) : Bool}:
  match xs:
    case Nil{}:
      {==}
    case +x <> +rest:
      +ap = S.approved(ops, x)
      +a1 = S.all_approved(ops, rest)
      +a2 = S.all_approved(ops, ys)
      Equal.trans(Bool, Bool.and(ap, S.all_approved(ops, List.append(&2, Nat, rest, ys))), Bool.and(ap, Bool.and(a1, a2)), Bool.and(Bool.and(ap, a1), a2), Equal.cong(Bool, Bool, z => Bool.and(ap, z), S.all_approved(ops, List.append(&2, Nat, rest, ys)), Bool.and(a1, a2), ih.all_app(ops, rest, ys)), ih.and_assoc(ap, a1, a2))

# a record changes no approval of the ids
def ih.all_hold(+j: Nat, +rest: List<&2, Ir.Op>, xs: List<&2, Nat>) -> {S.all_approved(Ir.Hold{j} <> rest, xs) == S.all_approved(rest, xs) : Bool}:
  match xs:
    case Nil{}:
      {==}
    case +x <> +more:
      %ih.all_hold(j, rest, more) : {S.all_approved(Ir.Hold{j} <> rest, x <> more) == Bool.and(S.approved(rest, x), _) : Bool}
      {==}

# p and q, with s or p and r: p, q and r
def ih.bool_lift(p: Bool, q: Bool, r: Bool, s: Bool) -> {Bool.and(Bool.and(p, q), Bool.and(Bool.or(s, p), r)) == Bool.and(p, Bool.and(q, r)) : Bool}:
  match p:
    case True{}:
      match s:
        case True{}:
          {==}
        case False{}:
          {==}
    case False{}:
      {==}

# an approval in front approves at least the ids approved without it
def ih.lift_eq(+w: Ir.Which, +rest: List<&2, Ir.Op>, xs: List<&2, Nat>) -> {Bool.and(S.all_approved(rest, xs), S.all_approved(Ir.Approve{w} <> rest, xs)) == S.all_approved(rest, xs) : Bool}:
  match xs:
    case Nil{}:
      {==}
    case +x <> +more:
      +ap = S.approved(rest, x)
      +q = S.all_approved(rest, more)
      +r = S.all_approved(Ir.Approve{w} <> rest, more)
      Equal.trans(Bool, Bool.and(Bool.and(ap, q), Bool.and(Bool.or(S.selects(w, x), ap), r)), Bool.and(ap, Bool.and(q, r)), Bool.and(ap, q), ih.bool_lift(ap, q, r, S.selects(w, x)), Equal.cong(Bool, Bool, z => Bool.and(ap, z), Bool.and(q, r), q, ih.lift_eq(w, rest, more)))

def ih.lift(+w: Ir.Which, +rest: List<&2, Ir.Op>, +xs: List<&2, Nat>, +h: {S.all_approved(rest, xs) == True{} : Bool}) -> {S.all_approved(Ir.Approve{w} <> rest, xs) == True{} : Bool}:
  +r = S.all_approved(Ir.Approve{w} <> rest, xs)
  +q = S.all_approved(rest, xs)
  Equal.trans(Bool, Bool.and(True{}, r), Bool.and(q, r), True{}, Equal.cong(Bool, Bool, z => Bool.and(z, r), True{}, q, Equal.sym(Bool, q, True{}, h)), Equal.trans(Bool, Bool.and(q, r), q, True{}, ih.lift_eq(w, rest, xs), h))

# every id an approval chooses is approved by it: one step
def ih.chosen.at(+w: Ir.Which, +rest: List<&2, Ir.Op>, +x: Nat, +c: List<&2, Nat>, b: Bool, eb: {S.selects(w, x) == b : Bool}, ih: {S.all_approved(Ir.Approve{w} <> rest, c) == True{} : Bool}) -> {S.all_approved(Ir.Approve{w} <> rest, S.keep(b, x, c)) == True{} : Bool}:
  match b:
    case True{}:
      %Equal.sym(Bool, S.selects(w, x), True{}, eb) : {Bool.and(Bool.or(_, S.approved(rest, x)), S.all_approved(Ir.Approve{w} <> rest, c)) == True{} : Bool}
      ih
    case False{}:
      ih

def ih.chosen(+w: Ir.Which, +rest: List<&2, Ir.Op>, xs: List<&2, Nat>) -> {S.all_approved(Ir.Approve{w} <> rest, S.chosen(w, xs)) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case +x <> +more:
      ih.chosen.at(w, rest, x, S.chosen(w, more), S.selects(w, x), {==}, ih.chosen(w, rest, more))

def ie.nat(+k: Nat, +rest: String, h: {LawsBase.wire_fits(k) == True{} : Bool}) -> {Wr.Got{k, rest} == Wr.get_nat(Wr.put_nat(k, rest)) : Wr.Dec<Nat>}:
  Equal.sym(Wr.Dec<Nat>, Wr.get_nat(Wr.put_nat(k, rest)), Wr.Got{k, rest}, Wire.wire_nat_round_trip(k, rest, h))

def ie.ns(+xs: List<&2, Nat>, +rest: String, h: {Irreversible.ir_ns_fit(xs) == True{} : Bool}) -> {Wr.Got{xs, rest} == Wr.get_list(~Nat, ~Wr.get_nat, Wr.put_list(~Nat, ~Wr.put_nat, xs, rest)) : Wr.Dec<List<&2, Nat>>}:
  Equal.sym(Wr.Dec<List<&2, Nat>>, Wr.get_list(~Nat, ~Wr.get_nat, Wr.put_list(~Nat, ~Wr.put_nat, xs, rest)), Wr.Got{xs, rest}, Wire.wire_list_round_trip(xs, rest, h))

# the entry's pairs of a ledger are the laws'
def ie.flat(es: List<&2, Ir.Entry>) -> {FIr.flat(es) == Irreversible.ir_flat(es) : List<&2, Nat>}:
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, st}:
          match st:
            case Ir.Waiting{}:
              %ie.flat(rest) : {FIr.flat(Ir.Entry{i, Ir.Waiting{}} <> rest) == i <> (0n <> _) : List<&2, Nat>}
              {==}
            case Ir.Taken{}:
              %ie.flat(rest) : {FIr.flat(Ir.Entry{i, Ir.Taken{}} <> rest) == i <> (1n <> _) : List<&2, Nat>}
              {==}

# the pairs of a ledger read back as the ledger
def ie.pairs(es: List<&2, Ir.Entry>) -> {FIr.pairs(Irreversible.ir_flat(es)) == Some{es} : Maybe<&2, List<&2, Ir.Entry>>}:
  match es:
    case Nil{}:
      {==}
    case e <> +rest:
      match e:
        case Ir.Entry{+i, st}:
          match st:
            case Ir.Waiting{}:
              %Equal.sym(Maybe<&2, List<&2, Ir.Entry>>, FIr.pairs(Irreversible.ir_flat(rest)), Some{rest}, ie.pairs(rest)) : {FIr.pairs.cons(i, Some{Ir.Waiting{}}, _) == Some{Ir.Entry{i, Ir.Waiting{}} <> rest} : Maybe<&2, List<&2, Ir.Entry>>}
              {==}
            case Ir.Taken{}:
              %Equal.sym(Maybe<&2, List<&2, Ir.Entry>>, FIr.pairs(Irreversible.ir_flat(rest)), Some{rest}, ie.pairs(rest)) : {FIr.pairs.cons(i, Some{Ir.Taken{}}, _) == Some{Ir.Entry{i, Ir.Taken{}} <> rest} : Maybe<&2, List<&2, Ir.Entry>>}
              {==}

# a spelled ledger reads back
def ie.ledger(+es: List<&2, Ir.Entry>, +rest: String, h: {Irreversible.ir_ns_fit(Irreversible.ir_flat(es)) == True{} : Bool}) -> {FIr.get_ledger(Irreversible.ir_w_ledger(es, rest)) == Wr.Got{es, rest} : Wr.Dec<List<&2, Ir.Entry>>}:
  %ie.ns(Irreversible.ir_flat(es), rest, h) : {FIr.get_ledger.go(_) == Wr.Got{es, rest} : Wr.Dec<List<&2, Ir.Entry>>}
  %Equal.sym(Maybe<&2, List<&2, Ir.Entry>>, FIr.pairs(Irreversible.ir_flat(es)), Some{es}, ie.pairs(es)) : {FIr.get_ledger.of(_, rest) == Wr.Got{es, rest} : Wr.Dec<List<&2, Ir.Entry>>}
  {==}

def ie.put_ledger(+es: List<&2, Ir.Entry>, +rest: String) -> {FIr.put_ledger(es, rest) == Irreversible.ir_w_ledger(es, rest) : String}:
  %ie.flat(es) : {FIr.put_ledger(es, rest) == Irreversible.ir_w_ns(_, rest) : String}
  {==}

# Route and settle
# ----------------

def ie.put_route(r: Ir.Route) -> {FIr.put_route(r) == Irreversible.ir_w_route_reply(r) : String}:
  match r:
    case Ir.Runs{}:
      {==}
    case Ir.Waits{}:
      {==}
    case Ir.Asks{}:
      {==}

def ie.put_approved(a: Ir.Approved) -> {FIr.put_approved(a) == Irreversible.ir_w_ns(Ir.Approved.run(a), Irreversible.ir_w_ledger(Ir.Approved.ledger(a), "")) : String}:
  match a:
    case Ir.Approved{+r, +l}:
      %ie.put_ledger(l, "") : {FIr.put_approved(Ir.Approved{r, l}) == Irreversible.ir_w_ns(r, _) : String}
      {==}

def ie.approve_tail(+w: Ir.Which, +es: List<&2, Ir.Entry>, fl: {Irreversible.ir_ns_fit(Irreversible.ir_flat(es)) == True{} : Bool}) -> {FIr.ffi.got(FIr.get_approve(Wr.Got{w, Irreversible.ir_w_ledger(es, "")})) == Irreversible.ir_w_ns(Ir.Approved.run(Ir.approve(w, es)), Irreversible.ir_w_ledger(Ir.Approved.ledger(Ir.approve(w, es)), "")) : String}:
  %Equal.sym(Wr.Dec<List<&2, Ir.Entry>>, FIr.get_ledger(Irreversible.ir_w_ledger(es, "")), Wr.Got{es, ""}, ie.ledger(es, "", fl)) : {FIr.ffi.got(FIr.get_approve.ledger(w, _)) == Irreversible.ir_w_ns(Ir.Approved.run(Ir.approve(w, es)), Irreversible.ir_w_ledger(Ir.Approved.ledger(Ir.approve(w, es)), "")) : String}
  ie.put_approved(Ir.approve(w, es))

def ie.which(+w: Ir.Which, +rest: String, fw: {Irreversible.ir_which_fit(w) == True{} : Bool}) -> {FIr.get_which(Irreversible.ir_w_which(w, rest)) == Wr.Got{w, rest} : Wr.Dec<Ir.Which>}:
  match w:
    case Ir.All{}:
      {==}
    case Ir.Ids{+ks}:
      %ie.ns(ks, rest, fw) : {FIr.get_which.ids(_) == Wr.Got{Ir.Ids{ks}, rest} : Wr.Dec<Ir.Which>}
      {==}

# Errors
# ------

def ie.ns_ok(+xs: List<&2, Nat>, +rest: String) -> {LawsBase.wire_opens_err(FIr.put_ns(xs, rest)) == False{} : Bool}:
  %Equal.sym(String, Wr.put_list(~Nat, ~Wr.put_nat, xs, rest), LawsBase.wire_spell_nats(xs, rest), Wire.wire_nats_spells(xs, rest)) : {LawsBase.wire_opens_err(_) == False{} : Bool}
  Wire.wire_nat_not_err(List.length(&2, Nat, xs), LawsBase.wire_spell_nats.items(xs, rest))

def ie.route_ok(r: Ir.Route) -> {LawsBase.wire_opens_err(FIr.put_route(r)) == False{} : Bool}:
  match r:
    case Ir.Runs{}:
      {==}
    case Ir.Waits{}:
      {==}
    case Ir.Asks{}:
      {==}

def ie.approved_ok(a: Ir.Approved) -> {LawsBase.wire_opens_err(FIr.put_approved(a)) == False{} : Bool}:
  match a:
    case Ir.Approved{+r, l}:
      ie.ns_ok(r, FIr.put_ledger(l, ""))

def ie.answer_ok(q: FIr.Req) -> {LawsBase.wire_opens_err(FIr.answer(q)) == False{} : Bool}:
  match q:
    case FIr.QRoute{+auto, +effs, m}:
      ie.route_ok(Ir.route(effs, auto, m))
    case FIr.QSettle{sd}:
      match sd:
        case Ir.Yes{}:
          {==}
        case Ir.No{}:
          {==}
        case Ir.Silent{}:
          {==}
    case FIr.QHold{es, +i}:
      ie.ns_ok(FIr.flat(Ir.hold(es, i)), "")
    case FIr.QApprove{+w, es}:
      ie.approved_ok(Ir.approve(w, es))
    case FIr.QWaiting{es}:
      ie.ns_ok(Ir.waiting(es), "")

def ie.refuse(d: Wr.Dec<FIr.Req>, bad: {LawsBase.wire_opens_err(FIr.ffi.got(d)) == True{} : Bool}) -> {FIr.ffi.got(d) == Wr.err("irreversible: malformed request") : String}:
  match d:
    case Wr.Got{+q, rest}:
      match rest:
        case SNil{}:
          irb.absurd_tf({FIr.answer(q) == Wr.err("irreversible: malformed request") : String}, Equal.trans(Bool, True{}, LawsBase.wire_opens_err(FIr.answer(q)), False{}, Equal.sym(Bool, LawsBase.wire_opens_err(FIr.answer(q)), True{}, bad), ie.answer_ok(q)))
        case SCon{x, y}:
          {==}
    case Wr.Bad{}:
      {==}
