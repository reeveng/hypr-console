These are handcrafted embedded JBIG2 streams containing a page information
segment, a 16 by 16 immediate generic region, and an end-of-page segment.
They contain no external artwork. The generic regions differ only in their
template flags and default adaptive pixel offsets (templates 0 through 3).

Their common MQ payload is:

```
84c73bfce1a1430402200000410dbb86f4317fff88ff37471adb6adfffac
```

The `.pixels` files record each decoded row (1 is black) from the original
fork decoder before extraction. Tests compare every pixel and reuse the
same document decoder context across images and repeated decodes.
