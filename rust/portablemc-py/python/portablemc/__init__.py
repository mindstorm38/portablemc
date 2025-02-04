# Reimport from our native module '_portablemc'.
from ._portablemc import standard, mojang  # type: ignore

# Our native module
import sys
sys.modules["portablemc.standard"] = standard
sys.modules["portablemc.mojang"] = mojang
del sys
