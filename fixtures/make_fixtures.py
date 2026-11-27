"""Generate test .docx fixtures with python-docx (pip install python-docx pillow)."""
import io, sys, os
from docx import Document
from docx.shared import Pt, RGBColor, Inches
from docx.enum.text import WD_ALIGN_PARAGRAPH, WD_BREAK
from docx.oxml.ns import qn
from docx.oxml import OxmlElement

out = os.path.dirname(os.path.abspath(__file__))

def png(w=120, h=60, color=(70, 110, 220)):
    try:
        from PIL import Image
        im = Image.new("RGB", (w, h), color)
        b = io.BytesIO(); im.save(b, "PNG"); b.seek(0); return b
    except ImportError:
        return None

def add_hyperlink(paragraph, url, text):
    part = paragraph.part
    r_id = part.relate_to(url, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", is_external=True)
    h = OxmlElement("w:hyperlink"); h.set(qn("r:id"), r_id)
    r = OxmlElement("w:r"); rpr = OxmlElement("w:rPr")
    st = OxmlElement("w:rStyle"); st.set(qn("w:val"), "Hyperlink"); rpr.append(st)
    r.append(rpr); t = OxmlElement("w:t"); t.text = text; r.append(t); h.append(r)
    paragraph._p.append(h)

def add_page_field(paragraph):
    for kind, text in (("begin", None), ("instr", " PAGE "), ("separate", None), ("result", "1"), ("end", None)):
        r = OxmlElement("w:r")
        if kind == "instr":
            it = OxmlElement("w:instrText"); it.set(qn("xml:space"), "preserve"); it.text = text; r.append(it)
        elif kind == "result":
            t = OxmlElement("w:t"); t.text = text; r.append(t)
        else:
            fc = OxmlElement("w:fldChar"); fc.set(qn("w:fldCharType"), kind); r.append(fc)
        paragraph._p.append(r)

def rich():
    d = Document()
    d.core_properties.title = "Docxtor fixture"
    d.add_heading("Quarterly Product Report", 0)
    p = d.add_paragraph("This report summarises ")
    p.add_run("bold progress").bold = True
    p.add_run(", ")
    r = p.add_run("italic caveats"); r.italic = True
    p.add_run(" and ")
    r = p.add_run("red underlined risks"); r.underline = True; r.font.color.rgb = RGBColor(0xC0, 0, 0)
    p.add_run(". Visit ")
    add_hyperlink(p, "https://example.com/report", "the dashboard")
    p.add_run(" for live numbers.")
    d.add_heading("Highlights", 1)
    for t in ["Revenue grew 18% year over year", "Two new enterprise customers", "Churn down to 2.1%"]:
        d.add_paragraph(t, style="List Bullet")
    d.add_paragraph("Nested detail", style="List Bullet 2")
    d.add_heading("Plan", 2)
    for t in ["Ship the editor", "Publish fidelity scores", "Launch AI panel"]:
        d.add_paragraph(t, style="List Number")
    q = d.add_paragraph("Simplicity is the ultimate sophistication.", style="Quote")
    c = d.add_paragraph("Centered, 14pt Georgia text."); c.alignment = WD_ALIGN_PARAGRAPH.CENTER
    c.runs[0].font.size = Pt(14); c.runs[0].font.name = "Georgia"
    d.add_heading("Numbers", 1)
    t = d.add_table(rows=4, cols=3); t.style = "Table Grid"
    data = [("Metric", "Q2", "Q3"), ("Revenue", "$1.2M", "$1.4M"), ("Users", "12,400", "15,900"), ("NPS", "41", "47")]
    for i, row in enumerate(data):
        for j, v in enumerate(row):
            t.cell(i, j).text = v
            if i == 0:
                t.cell(i, j).paragraphs[0].runs[0].bold = True
    m = t.cell(3, 1).merge(t.cell(3, 2)); m.text = "Merged NPS 41 → 47"
    img = png()
    if img:
        d.add_paragraph("A chart placeholder image:")
        d.add_picture(img, width=Inches(2))
    p = d.add_paragraph("Emoji and symbols: 😀 café ñ — “quotes” & <angle> brackets.\tTabbed.")
    p.add_run().add_break(WD_BREAK.LINE)
    p.add_run("After a line break.")
    d.add_page_break()
    d.add_heading("Appendix", 1)
    d.add_paragraph("Final paragraph on page two.")
    sec = d.sections[0]
    sec.header.paragraphs[0].text = "Docxtor — confidential draft"
    fp = sec.footer.paragraphs[0]; fp.text = "Page "; fp.alignment = WD_ALIGN_PARAGRAPH.CENTER
    add_page_field(fp)
    d.save(os.path.join(out, "rich.docx"))

def simple():
    d = Document()
    d.add_paragraph("Hello world.")
    d.save(os.path.join(out, "simple.docx"))

def long_doc():
    d = Document()
    d.add_heading("Long document", 0)
    for i in range(1, 41):
        d.add_heading(f"Section {i}", 1)
        for j in range(3):
            d.add_paragraph(f"Paragraph {j+1} of section {i}. " + "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " * 4)
    d.save(os.path.join(out, "long.docx"))

rich(); simple(); long_doc()
print("fixtures written to", out)


def complex_constructs():
    """Constructs Docxtor preserves but doesn't edit yet: tracked changes, bookmarks,
    content controls, fields, math, symbols. They must survive a round trip untouched."""
    from docx.oxml import parse_xml
    W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"'
    d = Document()
    d.add_heading("Complex constructs", 1)
    body = d.element.body
    sect = body[-1]
    xml = [
        f'<w:sdt {W}><w:sdtPr><w:docPartObj><w:docPartGallery w:val="Table of Contents"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent>'
        '<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \\o "1-3" \\h \\z \\u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Complex constructs ........ 1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>'
        '</w:sdtContent></w:sdt>',
        f'<w:p {W}><w:bookmarkStart w:id="0" w:name="intro"/><w:r><w:t xml:space="preserve">Tracked: </w:t></w:r>'
        '<w:ins w:id="1" w:author="Ada" w:date="2026-01-01T00:00:00Z"><w:r><w:t>inserted text</w:t></w:r></w:ins>'
        '<w:del w:id="2" w:author="Ada" w:date="2026-01-01T00:00:00Z"><w:r><w:delText>deleted text</w:delText></w:r></w:del>'
        '<w:r><w:t xml:space="preserve"> and a bookmark end.</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>',
        f'<w:p {W}><w:r><w:t xml:space="preserve">Simple field: page </w:t></w:r><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>'
        '<w:r><w:t xml:space="preserve">, symbol </w:t></w:r><w:r><w:sym w:font="Wingdings" w:char="F0FC"/></w:r>'
        '<w:r><w:t xml:space="preserve">, equation </w:t></w:r><m:oMath><m:r><m:t>E=m</m:t></m:r><m:sSup><m:e><m:r><m:t>c</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:oMath></w:p>',
        f'<w:p {W}><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="1F6F5C"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="F3F1EC"/></w:pPr>'
        '<w:r><w:t>Shaded paragraph with a bottom border.</w:t></w:r><w:customXmlInsRangeStart w:id="9" w:author="Ada" w:date="2026-01-01T00:00:00Z"/><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>sup</w:t></w:r><w:customXmlInsRangeEnd w:id="9"/></w:p>',
    ]
    for x in xml:
        sect.addprevious(parse_xml(x))
    d.add_paragraph("Final paragraph.")
    d.save(os.path.join(out, "complex.docx"))

complex_constructs()
