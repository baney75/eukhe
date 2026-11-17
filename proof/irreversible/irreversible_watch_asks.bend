# eukhe: the proof of LAW irreversible_watch_asks

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

def Irreversible.irreversible_watch_asks(effs, auto, g):
  %Equal.sym(Ir.Route, Ir.route(effs, auto, Ir.Watch{}), S.route(effs, auto, Ir.Watch{}), Irreversible.irreversible_route_is_spec(effs, auto, Ir.Watch{})) : {_ == Ir.Asks{} : Ir.Route}
  %Equal.sym(Bool, S.in_grant(effs, auto), False{}, g) : {S.route.of(_, Ir.Watch{}) == Ir.Asks{} : Ir.Route}
  {==}
