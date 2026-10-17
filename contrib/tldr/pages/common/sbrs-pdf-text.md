# sbrs pdf-text

> Print the text extracted from a page or PDF URL.
> Needs a build with the `pdf` Cargo feature. Without it, the command exits with status 2.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/pdf_parsing>.

- Print the text of a PDF:

`sbrs pdf-text {{https://example.com/file.pdf}}`

- Print the text of a web page:

`sbrs pdf-text {{https://example.com}}`

- Print the text and also keep the intermediate PDF:

`sbrs pdf-text {{https://example.com}} --save-pdf {{path/to/file.pdf}}`
