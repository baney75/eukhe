# eukhe: the proof of LAW irreversible_route_is_spec

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

def Irreversible.irreversible_route_is_spec(effs, auto, m):
  %IRB.irb.within_in_grant(effs, auto) : {Ir.route(effs, auto, m) == S.route.of(_, m) : Ir.Route}
  IRB.irb.route_of(Tl.within(effs, auto), m)
