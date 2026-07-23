from __future__ import annotations

import mimetypes
from dataclasses import dataclass
from io import IOBase
from os import PathLike
from pathlib import Path
from typing import TypeAlias

MediaInput: TypeAlias = str | PathLike[str] | bytes | bytearray | memoryview | IOBase


@dataclass(frozen=True, slots=True)
class MediaFile:
    file: MediaInput
    name: str | None = None
    mime: str | None = None


MediaFilesInput: TypeAlias = (
    MediaInput
    | MediaFile
    | list[MediaInput | MediaFile]
    | tuple[MediaInput | MediaFile, ...]
)


def normalize(
    value: MediaFilesInput,
    *,
    name: str | None,
    mime: str | None,
) -> list[MediaFile]:
    values = value if isinstance(value, (list, tuple)) else [value]
    if len(values) > 1 and name is not None:
        raise ValueError("name cannot be used with multiple files")
    return [_normalize_one(item, name=name, mime=mime) for item in values]


def read(file: MediaFile) -> bytes:
    value = file.file
    if isinstance(value, bytes):
        return value
    if isinstance(value, (bytearray, memoryview)):
        return bytes(value)
    if isinstance(value, (str, PathLike)):
        return Path(value).read_bytes()
    data = value.read()
    if not isinstance(data, (bytes, bytearray)):
        raise TypeError("media streams must return bytes")
    return bytes(data)


def _normalize_one(
    value: MediaInput | MediaFile,
    *,
    name: str | None,
    mime: str | None,
) -> MediaFile:
    if isinstance(value, MediaFile):
        if name is not None or mime is not None:
            raise ValueError("name and mime cannot be combined with MediaFile")
        return value
    inferred_name = Path(value).name if isinstance(value, (str, PathLike)) else None
    final_name = name or inferred_name
    final_mime = mime or (
        mimetypes.guess_type(final_name)[0] if final_name is not None else None
    )
    return MediaFile(value, final_name, final_mime)
