These fixtures are generated from a 16 by 16 grayscale image whose sample at
(x, y) is `(17*x + 29*y + 3*x*y) % 256`. They contain no external artwork.

OpenJPEG 2.5.4 encoded the PGM losslessly using:

```
opj_compress -i pattern.pgm -o ordinary.j2k -n 2 -b 4,4 -M 0
opj_compress -i pattern.pgm -o reset.j2k -n 2 -b 4,4 -M 2
opj_compress -i pattern.pgm -o reset-restart.j2k -n 2 -b 4,4 -M 6
```

The small blocks exercise new-block context initialization repeatedly. RESET
resets probability contexts after each pass; RESTART also restarts the MQ
engine after each pass. Tests compare every decoded sample to the formula.
