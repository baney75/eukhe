# eukhe: the proof of LAW irreversible_silence_runs_nothing

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

def Irreversible.irreversible_silence_runs_nothing(ops, es, quiet):
  match ops:
    case Nil{}:
      {==}
    case op <> +rest:
      match op:
        case Ir.Hold{i}:
          Irreversible.irreversible_silence_runs_nothing(rest, Ir.hold(es, i), quiet)
        case Ir.Approve{w}:
          IRB.irb.absurd_ft({Ir.Hist.ran(Ir.run(Ir.Approve{w} <> rest, es)) == [] : List<&2, Nat>}, quiet)
