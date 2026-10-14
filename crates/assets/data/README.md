# FMOD Vorbis setup headers

`fmod_vorbis_setups.bin` holds the 26 Vorbis setup packets (type 5) that the game's
FSB5 Vorbis clips reference by CRC32 (every clip of `loadedsoundeffects`,
`loadedsoundeffects_ambient`, `music_songs` and `music_procedural`). FMOD does not store
them in the banks.

- Source: vgmstream, `src/coding/libs/vorbis_codebooks_fsb.h` at commit
  `7c520e8f9b078dfde2f84f1b5bcbad0c6c415806` (2026-10-10), file SHA-256
  `f1e7a8c46de6babd0ed577bde0a13e8b7dab68dd9fd3082188b78dbc8057e752`. That file says
  the packets were extracted from FMOD by python-fsb5
  (<https://github.com/HearthSim/python-fsb5>).
- Licence: vgmstream's ISC-style licence, reproduced in `LICENSE-vgmstream`.
- Made with `make_fmod_vorbis_setups.py <vorbis_codebooks_fsb.h> <out> <crc,crc,...>`,
  which keeps the listed CRCs and checks each packet's CRC32.
- Format: little-endian `u32` count, then per entry `u32` CRC32, `u32` length and the
  packet, sorted by CRC.
