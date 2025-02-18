# Reimport from our native module '_portablemc'.
from ._portablemc import msa, standard, mojang, fabric, forge  # type: ignore

# Our native module
import sys
sys.modules["portablemc.msa"] = msa
sys.modules["portablemc.standard"] = standard
sys.modules["portablemc.mojang"] = mojang
sys.modules["portablemc.fabric"] = fabric
sys.modules["portablemc.forge"] = forge
del sys
