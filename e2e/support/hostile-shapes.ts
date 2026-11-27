// Hostile slots in drawings and colours: shapes, text boxes, WordArt, charts, the page, and a colour in every place one is read.
import { wordChartPart, wordSeries } from "./docx-builder";
import { W, R, REL, A, XML, esc, run, htmlPayload, evilUrl, cssFont } from "./hostile-payloads";
import type { Ctx } from "./hostile-docx";

/** shapes: colours, fonts and text of a shape, a text box, WordArt, a chart, the page and a table, as values that try to get out of CSS */
export function shapesSlot(c: Ctx): void {
  const { on, body, p, nextRid, t, para, addPart } = c;
  if (on.has("shapes")) {
    const bad = (id: string) => `FF0000;background:url(${evilUrl("shapes", id)})`;
    const clr = (id: string) => `<a:srgbClr val="${esc(bad(id))}"/>`;
    const wsp = (id: number, name: string, warp: string, inner: string) =>
      `<w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2400000" cy="1200000"/><wp:docPr id="${id}" name="${esc(name)}" descr="${esc(htmlPayload(`shapes:${name}`))}"/><wp:cNvGraphicFramePr/>` +
      `<a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><wps:cNvSpPr/>` +
      `<wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2400000" cy="1200000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill>${clr(`${name}-fill`)}</a:solidFill>` +
      `<a:ln w="12700"><a:solidFill>${clr(`${name}-line`)}</a:solidFill></a:ln><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000">${clr(`${name}-shadow`)}</a:outerShdw><a:glow rad="63500">${clr(`${name}-glow`)}</a:glow></a:effectLst></wps:spPr>` +
      `<wps:txbx><w:txbxContent><w:p><w:pPr><w:jc w:val="${esc(`center;background:url(${evilUrl("shapes", `${name}-jc`)})`)}"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="${esc(cssFont(`${name}-font`))}"/><w:color w:val="${esc(bad(`${name}-text`))}"/><w:sz w:val="${esc(`28;background:url(${evilUrl("shapes", `${name}-size`)})`)}"/>${inner}</w:rPr><w:t>${esc(htmlPayload(`shapes:${name}-text`))}</w:t></w:r></w:p></w:txbxContent></wps:txbx>` +
      `<wps:bodyPr>${warp}</wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`;
    body.push(para(wsp(c.docPrId++, "box", "", "")));
    body.push(para(wsp(c.docPrId++, "wordart", `<a:prstTxWarp prst="textArchUp"><a:avLst/></a:prstTxWarp>`, `<w14:textFill><w14:solidFill><w14:srgbClr val="${esc(bad("wordart-textfill"))}"/></w14:solidFill></w14:textFill><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr val="${esc(bad("wordart-outline"))}"/></w14:solidFill></w14:textOutline>`)));
    // a chart with colours, a title and series names that are payloads
    const cats = ["North", "South"];
    const ser = (i: number, name: string) =>
      wordSeries(i, name, [1, 2], cats, { spPr: `<c:spPr><a:solidFill>${clr(`chart-series-${i}`)}</a:solidFill><a:ln><a:solidFill>${clr(`chart-line-${i}`)}</a:solidFill></a:ln></c:spPr>` });
    const plot = `<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${ser(0, htmlPayload("shapes:series-0"))}${ser(1, `x" onmouseover="${run("shapes:series-1")}`)}<c:gapWidth val="219"/><c:axId val="111"/><c:axId val="222"/></c:barChart>` +
      `<c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="222"/></c:catAx><c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:crossAx val="111"/></c:valAx>`;
    addPart("word/charts/chart1.xml", wordChartPart(plot, { title: htmlPayload("shapes:chart-title"), legend: '<c:legend><c:legendPos val="b"/><c:overlay val="0"/></c:legend>' }), "application/vnd.openxmlformats-officedocument.drawingml.chart+xml");
    const chartRid = nextRid();
    p.rels.push(`<Relationship Id="${chartRid}" Type="${REL}/chart" Target="charts/chart1.xml"/>`);
    body.push(
      para(
        `<w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="4572000" cy="2743200"/><wp:docPr id="${c.docPrId++}" name="Chart" descr="${esc(htmlPayload("shapes:chart-alt"))}"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="${chartRid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`,
      ),
    );
    // a content control with a colour, a table whose borders and cells have payloads for colours
    body.push(
      `<w:sdt><w:sdtPr xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w:alias w:val="coloured"/><w:id w:val="4244"/><w15:color w:val="${esc(bad("control-color"))}"/><w:text/></w:sdtPr><w:sdtContent>${para(t("coloured control"))}</w:sdtContent></w:sdt>`,
    );
    body.push(
      `<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="${esc(bad("table-border"))}"/><w:insideH w:val="${esc(`single;background:url(${evilUrl("shapes", "border-val")})`)}" w:sz="4" w:color="000000"/></w:tblBorders><w:shd w:val="clear" w:color="auto" w:fill="${esc(bad("table-shd"))}"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcBorders><w:left w:val="single" w:sz="4" w:color="${esc(bad("cell-border"))}"/></w:tcBorders><w:shd w:val="pct25" w:color="${esc(bad("cell-pattern-color"))}" w:fill="${esc(bad("cell-fill"))}"/><w:vAlign w:val="${esc(`center;background:url(${evilUrl("shapes", "valign")})`)}"/></w:tcPr>${para(t("bordered cell"))}</w:tc></w:tr></w:tbl>`,
    );
    // the page: its colour, its borders
    c.documentBackground = `<w:background w:color="${esc(bad("page-background"))}"/>`;
    c.settingsExtra += "<w:displayBackgroundShape/>";
    c.sectionExtra = `<w:pgBorders w:offsetFrom="page"><w:top w:val="single" w:sz="8" w:space="24" w:color="${esc(bad("page-border"))}"/><w:left w:val="${esc(`single;background:url(${evilUrl("shapes", "page-border-val")})`)}" w:sz="8" w:space="24" w:color="000000"/></w:pgBorders>`;
  }
}

/** colours: a colour in every place the engine reads one, each a value that tries to get out of CSS, out of an attribute, or past a */
export function coloursSlot(c: Ctx): void {
  const { on, body, p, nextRid, t, para, addPart, cc, needPng } = c;
  // check that counts characters (see `badColour`). The colour the file meant never matters: the shape has no fill, the text its default.
  if (on.has("colours")) {
    const WNS14 = 'xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"';
    const WP = 'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"';
    const solid = (id: string) => `<a:solidFill><a:srgbClr val="${cc(id)}"/></a:solidFill>`;
    const rpr = (inner: string, text: string) => `<w:r ${WNS14}><w:rPr>${inner}</w:rPr><w:t xml:space="preserve">${esc(text)}</w:t></w:r>`;
    // text: colour, highlight, underline, box, shading, theme references, Word 2010 effects
    body.push(
      para(
        rpr(`<w:color w:val="${cc("run-color")}"/><w:highlight w:val="${cc("run-highlight")}"/><w:u w:val="single" w:color="${cc("run-underline")}"/><w:bdr w:val="single" w:sz="4" w:space="0" w:color="${cc("run-border")}"/><w:shd w:val="pct25" w:color="${cc("run-shd-color")}" w:fill="${cc("run-shd-fill")}"/>`, "colours: run"),
        `<w:pBdr><w:top w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-top")}"/><w:left w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-left")}"/><w:bottom w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-bottom")}"/><w:right w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-right")}"/><w:between w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-between")}"/><w:bar w:val="single" w:sz="4" w:space="1" w:color="${cc("para-border-bar")}"/></w:pBdr><w:shd w:val="pct25" w:color="${cc("para-shd-color")}" w:fill="${cc("para-shd-fill")}"/><w:rPr><w:color w:val="${cc("para-mark-color")}"/></w:rPr>`,
      ),
    );
    body.push(
      para(
        rpr(`<w:color w:val="${cc("theme-run-fallback")}" w:themeColor="${cc("theme-name")}" w:themeTint="${cc("theme-tint")}" w:themeShade="${cc("theme-shade")}"/><w:shd w:val="clear" w:color="auto" w:fill="${cc("theme-shd-fallback")}" w:themeFill="${cc("theme-fill-name")}" w:themeFillTint="${cc("theme-fill-tint")}" w:themeFillShade="${cc("theme-fill-shade")}"/><w:u w:val="single" w:color="${cc("theme-underline-fallback")}" w:themeColor="${cc("theme-underline-name")}" w:themeTint="${cc("theme-underline-tint")}"/>`, "colours: theme references"),
      ),
    );
    body.push(
      para(
        rpr(`<w:color w:val="${cc("tinted-run-fallback")}" w:themeColor="accent1" w:themeTint="${cc("tinted-tint")}" w:themeShade="${cc("tinted-shade")}"/><w:shd w:val="clear" w:color="auto" w:fill="${cc("tinted-shd-fallback")}" w:themeFill="accent2" w:themeFillTint="${cc("tinted-fill-tint")}" w:themeFillShade="${cc("tinted-fill-shade")}"/>`, "colours: tinted theme colours"),
      ),
    );
    body.push(
      para(
        rpr(
          `<w14:glow w14:rad="63500"><w14:srgbClr w14:val="${cc("fx-glow")}"/></w14:glow><w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl"><w14:srgbClr w14:val="${cc("fx-shadow")}"/></w14:shadow><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="${cc("fx-outline")}"/></w14:solidFill></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="${cc("fx-gradient-1")}"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="${cc("fx-gradient-2")}"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill>`,
          "colours: text effects",
        ),
      ),
    );
    body.push(para(rpr(`<w14:textFill><w14:solidFill><w14:srgbClr w14:val="${cc("fx-solid")}"/></w14:solidFill></w14:textFill>`, "colours: solid text fill")));
    // a table: borders, cell borders and diagonals, table and cell shading (a pattern, and a theme fill)
    const sides = (names: string[], tag: string) => names.map((n) => `<w:${n} w:val="single" w:sz="4" w:space="0" w:color="${cc(`${tag}-${n}`)}"/>`).join("");
    body.push(
      `<w:tbl><w:tblPr><w:tblBorders>${sides(["top", "left", "bottom", "right", "insideH", "insideV"], "table-border")}</w:tblBorders><w:shd w:val="clear" w:color="auto" w:fill="${cc("table-shd")}"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:tcBorders>${sides(["top", "left", "bottom", "right", "tl2br", "tr2bl"], "cell-border")}</w:tcBorders><w:shd w:val="pct25" w:color="${cc("cell-shd-color")}" w:fill="${cc("cell-shd-fill")}"/></w:tcPr>${para(t("colours: cell one"))}</w:tc><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="${cc("cell-themed-fallback")}" w:themeFill="${cc("cell-themed-name")}"/></w:tcPr>${para(t("colours: cell two"))}</w:tc></w:tr></w:tbl>`,
    );
    body.push(para(""));
    // a horizontal rule (VML), content controls with a colour
    body.push(para(`<w:r><w:pict><v:rect style="width:0;height:1.5pt" o:hr="t" o:hrstd="t" o:hralign="center" fillcolor="#${cc("hrule")}" stroked="f"/></w:pict></w:r>`));
    body.push(
      `<w:sdt><w:sdtPr xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w:alias w:val="coloured block"/><w:id w:val="4251"/><w15:color w:val="${cc("control-block")}"/><w:text/></w:sdtPr><w:sdtContent>${para(t("colours: block control"))}</w:sdtContent></w:sdt>`,
    );
    body.push(
      para(`<w:sdt><w:sdtPr xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w:alias w:val="coloured inline"/><w:id w:val="4252"/><w15:color w:val="${cc("control-inline")}"/><w:text/></w:sdtPr><w:sdtContent>${t("colours: inline control")}</w:sdtContent></w:sdt>`),
    );
    // shapes: solid fill and line, effects, a gradient (and a gradient line), a pattern, a theme style, WordArt, a freehand stroke
    const shape = (name: string, sppr: string, extra = "", txbxRpr = "", bodyPr = "") =>
      para(
        `<w:r ${WNS14}><w:drawing><wp:inline ${WP} distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2400000" cy="1200000"/><wp:docPr id="${c.docPrId++}" name="${esc(name)}"/><wp:cNvGraphicFramePr/>` +
          `<a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:cNvSpPr/>` +
          `<wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2400000" cy="1200000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>${sppr}</wps:spPr>${extra}` +
          `<wps:txbx><w:txbxContent><w:p><w:r><w:rPr>${txbxRpr}</w:rPr><w:t>${esc(name)}</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr>${bodyPr}</wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`,
      );
    body.push(
      shape(
        "colours: solid shape",
        `${solid("shape-fill")}<a:ln w="12700">${solid("shape-line")}</a:ln><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="${cc("shape-shadow")}"/></a:outerShdw><a:innerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="${cc("shape-inner-shadow")}"/></a:innerShdw><a:glow rad="63500"><a:srgbClr val="${cc("shape-glow")}"/></a:glow></a:effectLst>`,
        "",
        `<w:color w:val="${cc("shape-text")}"/>`,
      ),
    );
    body.push(
      shape(
        "colours: gradient shape",
        `<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="${cc("gradient-stop-1")}"/></a:gs><a:gs pos="50000"><a:schemeClr val="${cc("gradient-scheme")}"/></a:gs><a:gs pos="100000"><a:sysClr val="windowText" lastClr="${cc("gradient-system")}"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln w="12700"><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="${cc("line-gradient-1")}"/></a:gs><a:gs pos="100000"><a:srgbClr val="${cc("line-gradient-2")}"/></a:gs></a:gsLst></a:gradFill></a:ln>`,
      ),
    );
    body.push(
      shape(
        "colours: pattern shape",
        `<a:pattFill prst="pct50"><a:fgClr><a:srgbClr val="${cc("pattern-fg")}"/></a:fgClr><a:bgClr><a:srgbClr val="${cc("pattern-bg")}"/></a:bgClr></a:pattFill><a:ln w="12700"><a:solidFill><a:prstClr val="${cc("preset-colour")}"/></a:solidFill></a:ln>`,
      ),
    );
    body.push(
      shape(
        "colours: themed shape",
        "",
        `<wps:style><a:lnRef idx="2"><a:schemeClr val="${cc("style-line")}"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="${cc("style-fill")}"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="${cc("style-effect")}"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="${cc("style-font")}"/></a:fontRef></wps:style>`,
      ),
    );
    body.push(
      shape(
        "colours: wordart",
        "",
        "",
        `<w:color w:val="${cc("wordart-colour")}"/><w14:glow w14:rad="63500"><w14:srgbClr w14:val="${cc("wordart-glow")}"/></w14:glow><w14:shadow w14:blurRad="0" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl"><w14:srgbClr w14:val="${cc("wordart-shadow")}"/></w14:shadow><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="${cc("wordart-outline")}"/></w14:solidFill></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="${cc("wordart-gradient-1")}"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="${cc("wordart-gradient-2")}"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill>`,
        `<a:prstTxWarp prst="textArchUp"><a:avLst/></a:prstTxWarp>`,
      ),
    );
    // what the pen writes: a stroked curve
    body.push(
      shape("colours: ink", `<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="100" h="100" fill="none"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="100"/></a:lnTo></a:path></a:pathLst></a:custGeom><a:noFill/><a:ln w="19050" cap="rnd"><a:solidFill><a:srgbClr val="${cc("ink")}"><a:alpha val="50000"/></a:srgbClr></a:solidFill></a:ln>`),
    );
    // VML: fill and stroke by attribute, a gradient, a shadow
    body.push(para(`<w:r><w:pict><v:rect id="vml_a" style="width:100pt;height:50pt" fillcolor="#${cc("vml-fill")}" strokecolor="#${cc("vml-stroke")}"/></w:pict></w:r>`));
    body.push(para(`<w:r><w:pict><v:rect id="vml_b" style="width:100pt;height:50pt"><v:fill type="gradient" color="#${cc("vml-gradient-1")}" color2="#${cc("vml-gradient-2")}" colors="0 #${cc("vml-colors-1")};1 #${cc("vml-colors-2")}"/><v:stroke color="#${cc("vml-stroke-element")}"/><v:shadow on="t" color="#${cc("vml-shadow")}"/></v:rect></w:pict></w:r>`));
    // a chart: series, points, markers, titles, axes, gridlines, plot area, the chart's own fill
    const cats = ["North", "South"];
    const sp = (id: string) => `<c:spPr>${solid(`${id}-fill`)}<a:ln>${solid(`${id}-line`)}</a:ln></c:spPr>`;
    const txpr = (id: string) => `<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr>${solid(id)}</a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>`;
    const ser = (i: number) =>
      wordSeries(i, `Series ${i}`, [1, 2], cats, {
        spPr: sp(`chart-series-${i}`),
        extra: `<c:invertIfNegative val="0"/><c:dPt><c:idx val="1"/><c:invertIfNegative val="0"/><c:bubble3D val="0"/>${sp(`chart-point-${i}`)}</c:dPt><c:dLbls>${txpr(`chart-label-${i}`)}<c:showLegendKey val="0"/><c:showVal val="1"/><c:showCatName val="0"/><c:showSerName val="0"/><c:showPercent val="0"/><c:showBubbleSize val="0"/></c:dLbls>`,
      });
    const plot =
      `<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${ser(0)}${ser(1)}<c:gapWidth val="219"/><c:axId val="311"/><c:axId val="322"/></c:barChart>` +
      `<c:catAx><c:axId val="311"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/>${txpr("chart-axis-text")}<c:crossAx val="322"/></c:catAx><c:valAx><c:axId val="322"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines><c:spPr><a:ln>${solid("chart-gridline")}</a:ln></c:spPr></c:majorGridlines><c:crossAx val="311"/></c:valAx>${sp("chart-plot")}`;
    addPart("word/charts/chart2.xml", wordChartPart(plot, { title: "colours chart", legend: `<c:legend><c:legendPos val="b"/>${txpr("chart-legend")}</c:legend>`, extra: sp("chart-space") }), "application/vnd.openxmlformats-officedocument.drawingml.chart+xml");
    const chartRid = nextRid();
    p.rels.push(`<Relationship Id="${chartRid}" Type="${REL}/chart" Target="charts/chart2.xml"/>`);
    body.push(
      para(
        `<w:r><w:drawing><wp:inline ${WP} distT="0" distB="0" distL="0" distR="0"><wp:extent cx="4572000" cy="2743200"/><wp:docPr id="${c.docPrId++}" name="Colours chart"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="${chartRid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`,
      ),
    );
    // a picture with an outline, effects, a recolour and a colour change
    const picRid = needPng("colours.png");
    body.push(
      para(
        `<w:r><w:drawing><wp:inline ${WP} distT="0" distB="0" distL="0" distR="0"><wp:extent cx="914400" cy="914400"/><wp:docPr id="${c.docPrId++}" name="Colours picture"/><wp:cNvGraphicFramePr/>` +
          `<a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="colours.png"/><pic:cNvPicPr/></pic:nvPicPr>` +
          `<pic:blipFill><a:blip r:embed="${picRid}"><a:duotone><a:srgbClr val="${cc("picture-duotone-1")}"/><a:srgbClr val="${cc("picture-duotone-2")}"/></a:duotone><a:clrChange><a:clrFrom><a:srgbClr val="${cc("picture-change-from")}"/></a:clrFrom><a:clrTo><a:srgbClr val="${cc("picture-change-to")}"><a:alpha val="0"/></a:srgbClr></a:clrTo></a:clrChange></a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill>` +
          `<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:ln w="12700">${solid("picture-outline")}</a:ln><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="${cc("picture-shadow")}"/></a:outerShdw><a:glow rad="63500"><a:srgbClr val="${cc("picture-glow")}"/></a:glow></a:effectLst></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`,
      ),
    );
    // styles: a paragraph style, a character style and a table style with colours; the list label colours are in numbering.xml
    const side = (n: string, id: string) => `<w:${n} w:val="single" w:sz="4" w:space="0" w:color="${cc(id)}"/>`;
    c.stylesXml +=
      `<w:style w:type="paragraph" w:styleId="ColoursPara"><w:name w:val="Colours para"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:pBdr>${side("top", "style-border-top")}${side("bottom", "style-border-bottom")}</w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="${cc("style-para-shd")}"/></w:pPr><w:rPr><w:color w:val="${cc("style-color")}"/><w:highlight w:val="${cc("style-highlight")}"/><w:u w:val="single" w:color="${cc("style-underline")}"/></w:rPr></w:style>` +
      `<w:style w:type="character" w:styleId="ColoursChar"><w:name w:val="Colours char"/><w:rPr><w:color w:val="${cc("char-style-color")}"/><w:shd w:val="clear" w:color="auto" w:fill="${cc("char-style-shd")}"/></w:rPr></w:style>` +
      `<w:style w:type="table" w:styleId="ColoursTable"><w:name w:val="Colours table"/><w:tblPr><w:tblBorders>${side("top", "tstyle-border-top")}${side("insideH", "tstyle-border-inside")}</w:tblBorders></w:tblPr>` +
      `<w:tblStylePr w:type="firstRow"><w:rPr><w:color w:val="${cc("tstyle-first-row-text")}"/></w:rPr><w:tcPr><w:tcBorders>${side("bottom", "tstyle-first-row-border")}</w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="${cc("tstyle-first-row-shd")}"/></w:tcPr></w:tblStylePr>` +
      `<w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="${cc("tstyle-band-shd")}"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="${cc("tstyle-first-col-shd")}"/></w:tcPr></w:tblStylePr></w:style>`;
    body.push(para(t("colours: paragraph style"), `<w:pStyle w:val="ColoursPara"/>`));
    body.push(para(`<w:r><w:rPr><w:rStyle w:val="ColoursChar"/></w:rPr><w:t>colours: character style</w:t></w:r>`));
    const cell = (s: string) => `<w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>${para(t(s))}</w:tc>`;
    body.push(
      `<w:tbl><w:tblPr><w:tblStyle w:val="ColoursTable"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr>${cell("head")}${cell("head")}</w:tr><w:tr>${cell("a")}${cell("b")}</w:tr><w:tr>${cell("c")}${cell("d")}</w:tr></w:tbl>`,
    );
    body.push(para(""));
    // the page: its colour (and a gradient fill), its borders; a header with a watermark whose colour is hostile
    c.documentBackground = `<w:background w:color="${cc("page-color")}"><v:background id="_x0000_s1025" o:bwmode="white" fillcolor="#${cc("page-fill")}"><v:fill type="gradient" color2="#${cc("page-fill-2")}" angle="-90" focus="100%" method="none"/></v:background></w:background>`;
    c.settingsExtra += "<w:displayBackgroundShape/>";
    c.sectionExtra = `<w:pgBorders w:offsetFrom="page"><w:top w:val="single" w:sz="8" w:space="24" w:color="${cc("page-border-top")}"/><w:left w:val="single" w:sz="8" w:space="24" w:color="${cc("page-border-left")}"/><w:bottom w:val="single" w:sz="8" w:space="24" w:color="${cc("page-border-bottom")}"/><w:right w:val="single" w:sz="8" w:space="24" w:color="${cc("page-border-right")}"/></w:pgBorders><w:titlePg/>`;
    const hid = nextRid();
    addPart(
      "word/header2.xml",
      `${XML}<w:hdr xmlns:w="${W}" xmlns:r="${R}" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:p><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="${cc("header-shd")}"/></w:pPr><w:r><w:rPr><w:color w:val="${cc("header-run")}"/></w:rPr><w:t>colours: header</w:t></w:r><w:r><w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject2" o:spid="_x0000_s2050" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412pt;height:247pt;rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="#${cc("watermark")}" stroked="f"><v:fill opacity=".5"/><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="DRAFT"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape></w:pict></w:r></w:p></w:hdr>`,
      "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml",
    );
    p.rels.push(`<Relationship Id="${hid}" Type="${REL}/header" Target="header2.xml"/>`);
    c.coloursHeader = `<w:headerReference w:type="first" r:id="${hid}"/>`;
  }
}
