# eukhe: the proof of LAW irreversible_default_runs_exactly_reversible

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

def Irreversible.irreversible_default_runs_exactly_reversible(effs, m):
  %IRB.irb.default_within(effs) : {Irreversible.ir_runs(Ir.route(effs, Am.default_auto(), m)) == _ : Bool}
  IRB.irb.runs_of(Tl.within(effs, Am.default_auto()), m)
