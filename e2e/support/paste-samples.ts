// Clipboard HTML as Word, Google Docs and web pages write it, for the paste tests.

export const PNG_DATA = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/** What Word puts on the clipboard for a few formatted paragraphs, a list and a table (with its style sheet and Office markers). */
export const WORD_HTML = `<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w="urn:schemas-microsoft-com:office:word" xmlns="http://www.w3.org/TR/REC-html40">
<head><meta http-equiv=Content-Type content="text/html; charset=utf-8">
<!--[if gte mso 9]><xml><w:WordDocument><w:View>Normal</w:View></w:WordDocument></xml><![endif]-->
<style>
<!--
 p.MsoNormal, li.MsoNormal, div.MsoNormal
	{margin-top:0in;margin-right:0in;margin-bottom:8.0pt;margin-left:0in;line-height:107%;font-size:11.0pt;font-family:"Calibri",sans-serif;}
p.MsoListParagraph
	{margin-top:0in;margin-right:0in;margin-bottom:8.0pt;margin-left:.5in;line-height:107%;font-size:11.0pt;font-family:"Calibri",sans-serif;}
@list l0:level1
	{mso-level-number-format:bullet;mso-level-text:\\F0B7;mso-level-tab-stop:.5in;mso-level-number-position:left;text-indent:-.25in;font-family:Symbol;}
-->
</style></head>
<body lang=EN-US style='tab-interval:.5in'>
<!--StartFragment-->
<p class=MsoNormal><b><span style='font-size:14.0pt;line-height:107%;font-family:"Arial",sans-serif;color:#C00000'>Bold red Arial</span></b><o:p></o:p></p>
<p class=MsoNormal align=center style='text-align:center'><i>Centered</i> and <u>underlined</u> and <span style='mso-highlight:yellow'>highlighted</span>, x<sup>2</sup> and H<sub>2</sub>O<o:p></o:p></p>
<p class=MsoListParagraphCxSpFirst style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><![endif]>First bullet<o:p></o:p></p>
<p class=MsoListParagraphCxSpLast style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><![endif]>Second bullet<o:p></o:p></p>
<table class=MsoTableGrid border=1 cellspacing=0 cellpadding=0 width=400 style='width:300.0pt;border-collapse:collapse;border:none'>
 <tr>
  <td width=200 valign=top style='width:150.0pt;border:solid windowtext 1.0pt;background:#D9E2F3;padding:0in 5.4pt 0in 5.4pt'><p class=MsoNormal><b>Head A</b></p></td>
  <td width=200 valign=top style='width:150.0pt;border:solid windowtext 1.0pt;border-left:none;padding:0in 5.4pt 0in 5.4pt'><p class=MsoNormal>Head B</p></td>
 </tr>
 <tr>
  <td colspan=2 style='width:300.0pt;border:solid windowtext 1.0pt;border-top:none;padding:0in 5.4pt'><p class=MsoNormal>Spans both</p></td>
 </tr>
</table>
<p class=MsoNormal><a href="https://example.com/page">A link</a> and a picture: <img width=96 height=48 src="file:///C:/Users/x/AppData/Local/Temp/msohtmlclip1/01/clip_image001.png" alt="Logo"></p>
<!--EndFragment-->
</body></html>`;

/** Google Docs: a heading, a styled run, a list with a nested item and a table, all inside its `docs-internal-guid` wrapper. */
export const GDOCS_HTML = `<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-8b0c9b7c-7fff-0c0b-8b0e-6a2d1f0f5a11"><h1 dir="ltr" style="line-height:1.38;margin-top:20pt;margin-bottom:6pt;"><span style="font-size:20pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Big heading</span></h1><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Georgia,serif;color:#1155cc;background-color:#ffff00;font-weight:700;font-style:italic;text-decoration:underline;vertical-align:baseline;white-space:pre-wrap;">Bold italic blue on yellow</span></p><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;"><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;" aria-level="1"><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;">Item one</span></p></li><li dir="ltr" style="list-style-type:disc;" aria-level="1"><p dir="ltr" role="presentation"><span>Item two</span></p></li><ul style="margin-top:0;margin-bottom:0;"><li dir="ltr" style="list-style-type:circle;" aria-level="2"><p dir="ltr" role="presentation"><span>Nested</span></p></li></ul></ul><br /><div dir="ltr" align="left"><table style="border:none;border-collapse:collapse;"><colgroup><col width="200" /><col width="150" /></colgroup><tbody><tr style="height:0pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;background-color:#cfe2f3;padding:5pt 5pt 5pt 5pt;"><p dir="ltr" style="line-height:1.2;text-align:center;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;">Cell A</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;"><p dir="ltr"><span>Cell B</span></p></td></tr></tbody></table></div></b>`;

/** A web page: a heading, a link, a picture, inline code and a block quote. */
export const WEB_HTML = `<html><body><h2>Section <em>two</em></h2><p>Plain paragraph with <a href="https://example.com/x">a link</a>, <code>code</code> &amp; &quot;entities&quot; &mdash; done.</p><blockquote>Quoted text</blockquote><p><img src="${PNG_DATA}" width="40" height="20" alt="dot"></p></body></html>`;

/** The RTF Word writes for two formatted runs. */
export const RTF = String.raw`{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fnil\fcharset0 Calibri;}{\f1\fnil\fcharset0 Arial;}}{\colortbl ;\red192\green0\blue0;}\viewkind4\uc1\pard\sa200\sl276\slmult1\f1\fs28\cf1\b Bold red Arial\b0\cf0\f0\fs22  and plain\par}`;
