# Application UI fallback font

Noto Sans SC Variable, upstream font bytes unchanged. SIL Open Font License 1.1.
Copyright 2014-2021 Adobe, Reserved Font Name: Source.
Original copyright and full license: OFL-NotoSansSC.txt.
Pinned upstream URLs, commit and SHA-256: provenance.json.

Windows UI uses the message font from SPI_GETNONCLIENTMETRICS first. Noto Sans SC
is embedded in the executable and extracted at startup into the user-local
Glint/font-cache directory for Slint's SLINT_FONT_PATH fallback chain. No font
is installed system-wide. The runtime does not download font data. Annotation
font defaults, picker and rasterization are separate and unchanged.
