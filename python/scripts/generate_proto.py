import os
from pathlib import Path
import sys

import grpc_tools
from grpc_tools import protoc


def main() -> None:
    project = Path(__file__).resolve().parents[1]
    repository = project.parent
    output = project / "src" / "airi" / "generated"
    proto = repository / "proto" / "arisa.proto"
    well_known_types = Path(grpc_tools.__file__).resolve().parent / "_proto"
    executable_dir = Path(sys.executable).parent
    plugin = executable_dir / "protoc-gen-python_betterproto2"
    os.environ["PATH"] = f"{executable_dir}{os.pathsep}{os.environ.get('PATH', '')}"

    result = protoc.main(
        [
            "grpc_tools.protoc",
            f"-I{proto.parent}",
            f"-I{well_known_types}",
            f"--plugin=protoc-gen-python_betterproto2={plugin}",
            f"--python_betterproto2_out={output}",
            "--python_betterproto2_opt=client_generation=async",
            str(proto),
        ]
    )
    if result != 0:
        raise SystemExit(result)


if __name__ == "__main__":
    main()
