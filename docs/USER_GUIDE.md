# CSVRescue — User Guide

CSVRescue makes a CSV or text export readable again and saves it in a form Excel will not damage.

## What goes wrong with CSV files
A CSV has no label for its encoding or its delimiter, so Excel guesses: accents turn into Ã© or ?, everything lands in column A because the file uses semicolons, 1,5 becomes a date, part numbers lose their leading zeros, 1234567890123456 becomes 1.23457E+15 and 1E5 becomes 100000.

## How the file is read
| Setting | How CSVRescue decides | Change it when |
|---|---|---|
| Encoding | Byte order mark (UTF-8, UTF-16, UTF-32); UTF-16 without one (most bytes are zero); valid UTF-8; otherwise Windows-1252 | Letters look wrong: pick Windows-1250/1251/1253/1254/1257, ISO-8859-1/2, Shift-JIS, GBK, Big5, EUC-KR or IBM437 |
| Delimiter | The one of comma, semicolon, tab and pipe that splits the first rows into the same number of columns (two or more) | Everything is in one column |
| Decimal mark | The marks the numbers use (a comma followed by one, two or four or more digits is a decimal comma; with no evidence a semicolon file is treated as European) | Prices are read wrong |
| Header | The first row is the header | The first row is data |
| Repair | Text saved as UTF-8 twice is repaired when the repair is clean, run by run | The repair changes letters that were right |

Quoted fields, doubled quotes and line breaks inside quotes are read correctly; CRLF, LF and CR line endings all work. Rows with a different number of columns, empty lines, stray quotation marks and a missing closing quote are reported.

## Text or number
A column is **Number** only if every value is a clean number: no leading zeros, at most 15 digits, no plus sign, no 1E5 or date look. Decimal commas and thousands separators (1.234,56 / 1,234.56 / 1 234,5) are understood. Everything else is **Text** and is written exactly as it is. You can switch any column; a value that is not a number in a Number column is written as text, never dropped.

## Saving
- **Excel (.xlsx)**: Text cells have the Excel text format, numbers are real numbers, the header is bold and frozen, column widths fit. At most 1,048,576 rows.
- **CSV**: UTF-8 with or without a byte order mark (keep it for Excel); delimiter comma, semicolon (Excel in countries that use a decimal comma) or tab. *Write decimal commas as dots* converts Number columns for programs. *Protect risky Text cells for Excel* writes ="00123" for values that Excel would change; only Excel understands this form, other programs show the equals sign and quotes.
A plain CSV cannot carry the information 'this is text'; use the .xlsx file when it matters.

## The original is safe
CSVRescue reads the file you open and writes only the new file you save; it refuses to use the same name. Files up to 300 MB open.


## Problems
Errors are logged to `%AppData%\CSVRescue\error.log`. Open an [issue on GitHub](https://github.com/md-anamul-hasan/csvrescue/issues) and attach that file (remove anything private first).
