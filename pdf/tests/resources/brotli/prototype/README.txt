Brotli-Prototype-FileA.pdf
==========================

A real file written by a producer, not built by hand for this repository.

Source
------
https://github.com/mozilla/pdf.js/blob/master/test/pdfs/Brotli-Prototype-FileA.pdf
(md5 9113370932798776ba91c807ce95082e, 1,305,278 bytes — the same file)

Originally published by the PDF Association together with
https://pdfa.org/brotli-compression-coming-to-pdf/ in
https://pdfa.org/download-area/examples/Brotli-Prototype-PDFs.zip
("Brotli-Prototype-FileA.pdf", 30 Brotli compressed streams out of 30 streams),
and produced by their Brotli prototype implementations (Ghostscript and MuPDF).
The download area itself answers 403 to non-browser clients; the copy above is
the one pdf.js keeps in its test corpus.

What is in it
-------------
25 pages, all text and vector graphics (no images), PDF 2.0. Every stream in
the file is Brotli compressed: the 25 page content streams, the object stream
holding the non-stream objects and the cross-reference stream. A reader that
cannot decode BrotliDecode cannot even find the pages: the xref stream is the
first thing it has to inflate.

Tests
-----
pdf/tests/brotli_interop.rs, `prototype_file_from_the_pdf_association`:
opens the file, walks every page, loads every font resource and parses every
content stream (1,323,847 operators).

Licence
-------
The PDF Association publishes these as example files for implementers; no
explicit licence is stated. Keep the attribution above with the file.
