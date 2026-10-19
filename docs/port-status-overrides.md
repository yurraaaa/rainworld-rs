# Port status overrides

Hand-kept input of `scripts/port_status.py` (-> [`PORT-STATUS.md`](PORT-STATUS.md)). One line per type that is
mirrored but only partly ported; the generator shows it as `partial` with the reason. Format:

```
- Type: partial: reason
- Namespace/Type: partial: reason
```

`Type` is the decompiled file name without `.cs`; types in a namespace folder are written `HUD/Type`.
Only add a line when you know what is missing; an empty list means no type is marked partial.
