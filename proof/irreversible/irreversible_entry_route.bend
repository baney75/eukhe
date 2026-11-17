# eukhe: the proof of LAW irreversible_entry_route

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

def ie.route_mode(+auto: List<&2, Nat>, +effs: List<&2, Nat>, m: Ir.Mode) -> {FIr.ffi.got(FIr.get_route.tag(auto, effs, Wr.get_nat(Wr.put_nat(Irreversible.ir_mode_tag(m), "")))) == Irreversible.ir_w_route_reply(S.route(effs, auto, m)) : String}:
  match m:
    case Ir.End{}:
      %Irreversible.irreversible_route_is_spec(effs, auto, Ir.End{}) : {FIr.ffi.got(FIr.get_route.tag(auto, effs, Wr.get_nat(Wr.put_nat(0n, "")))) == Irreversible.ir_w_route_reply(_) : String}
      IRB.ie.put_route(Ir.route(effs, auto, Ir.End{}))
    case Ir.Watch{}:
      %Irreversible.irreversible_route_is_spec(effs, auto, Ir.Watch{}) : {FIr.ffi.got(FIr.get_route.tag(auto, effs, Wr.get_nat(Wr.put_nat(1n, "")))) == Irreversible.ir_w_route_reply(_) : String}
      IRB.ie.put_route(Ir.route(effs, auto, Ir.Watch{}))

def Irreversible.irreversible_entry_route(auto, effs, m, fa, fe):
  +r2 = Wr.put_list(~Nat, ~Wr.put_nat, effs, Wr.put_nat(Irreversible.ir_mode_tag(m), ""))
  +r1 = Wr.put_list(~Nat, ~Wr.put_nat, auto, r2)
  +rhs = Irreversible.ir_w_route_reply(S.route(effs, auto, m))
  %IRB.ie.nat(0n, r1, {==}) : {FIr.ffi.got(FIr.get_req.go(_)) == rhs : String}
  %IRB.ie.ns(auto, r2, fa) : {FIr.ffi.got(FIr.get_route(_)) == rhs : String}
  %IRB.ie.ns(effs, Wr.put_nat(Irreversible.ir_mode_tag(m), ""), fe) : {FIr.ffi.got(FIr.get_route.effs(auto, _)) == rhs : String}
  ie.route_mode(auto, effs, m)
