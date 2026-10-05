import os

with open('help_output.txt', 'r') as f:
    help_text = f.read()

append_str = f"""
---

## 10. Lampiran: Menu Bantuan Lengkap (`--help`)

Berikut adalah daftar keseluruhan argumen baris perintah (*Command Line Arguments*) yang didukung oleh SeisBox CFS:

```text
{help_text}
```
"""

with open('tutorial_cfs.md', 'a') as f:
    f.write(append_str)
