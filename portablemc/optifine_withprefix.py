from .optifine import *

class OptifineVersion(OptifineVersion):
    def __init__(self, version: str | None = None, prefix: str = "optifine", *args, context: Optional[Context] | None = None) -> None:
        super().__init__(version, prefix, *args, context=context)
        self.prefix = prefix

    def _resolve_version(self, watcher: Watcher) -> None:
        super()._resolve_version(watcher)
        edition=super().loader()
        mcver=super().mcver()
        self.version=f"{self.prefix}-{mcver}-{edition}"

    def mcver(self) -> str:
        return self.version.split("-")[1]

    def loader(self) -> str:
        return self.version.split("-")[2]

    def dl_url(self) -> str:
        edition=self.version.split("-")[2]
        mcver=self.version.split("-")[1]
        filename=""
        if re.match(r"pre\d",edition.split("_")[-1]):
            filename+="preview_"
        filename+="OptiFine_"+mcver+"_"+edition
        return f"http://optifine.net/download?f={filename}.jar"
