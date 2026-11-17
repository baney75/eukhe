# eukhe: the proof of LAW irreversible_granted_runs

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

def Irreversible.irreversible_granted_runs(effs, auto, m, g):
  %Equal.sym(Ir.Route, Ir.route(effs, auto, m), S.route(effs, auto, m), Irreversible.irreversible_route_is_spec(effs, auto, m)) : {_ == Ir.Runs{} : Ir.Route}
  %Equal.sym(Bool, S.in_grant(effs, auto), True{}, g) : {S.route.of(_, m) == Ir.Runs{} : Ir.Route}
  {==}
