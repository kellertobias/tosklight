// Render the German Impressum and Datenschutzerklärung of the public site from validated contact
// data. German is the legally binding version; each page closes with a short English summary that
// says so. The texts change only when the site, its hosting, or the law changes; update
// LEGAL_TEXT_DATE with them.

export const LEGAL_TEXT_DATE = "7. Oktober 2026";
export const IMPRESSUM_PATH = "impressum/";
export const DATENSCHUTZ_PATH = "datenschutz/";

const GITHUB_PRIVACY_STATEMENT =
	"https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement";
const HESSEN_DPA = "https://datenschutz.hessen.de/";

export const escapeHtml = (value) =>
	String(value)
		.replace(/&/gu, "&amp;")
		.replace(/</gu, "&lt;")
		.replace(/>/gu, "&gt;")
		.replace(/"/gu, "&quot;")
		.replace(/'/gu, "&#39;");

// Every character as a numeric reference: browsers show and follow the address normally, while
// scrapers that grep the raw HTML for "name@host" find nothing.
const encodeEntities = (value) =>
	[...value].map((character) => `&#x${character.codePointAt(0).toString(16)};`).join("");

export function emailLink(email) {
	return `<a href="${encodeEntities(`mailto:${email}`)}">${encodeEntities(email)}</a>`;
}

export function phoneLink(phone) {
	const dial = phone.replace(/\(0\)/gu, "").replace(/[^\d+]/gu, "");
	return `<a href="tel:${escapeHtml(dial)}">${escapeHtml(phone)}</a>`;
}

/** The footer every published page carries; `prefix` leads from the page back to the site root. */
export function legalLinks(prefix = "") {
	return (
		`<a href="${prefix}${IMPRESSUM_PATH}">Impressum</a> · ` +
		`<a href="${prefix}${DATENSCHUTZ_PATH}">Datenschutz</a>`
	);
}

const postalAddress = (contact) =>
	`${escapeHtml(contact.street)}<br />${escapeHtml(contact.postalCode)} ${escapeHtml(contact.city)}<br />${escapeHtml(contact.country)}`;

function page({ title, description, heading, lede, toc, body }) {
	const tocLinks = toc.map(([id, label]) => `<a href="#${id}">${escapeHtml(label)}</a>`).join("\n          ");
	return `<!doctype html>
<html lang="de">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="theme-color" content="#07090d" />
    <meta name="description" content="${escapeHtml(description)}" />
    <title>${escapeHtml(title)} — ToskLight</title>
    <link rel="icon" href="../icon.png" type="image/png" />
    <link rel="stylesheet" href="../site.css" />
  </head>
  <body class="legal-page">
    <a class="skip-link" href="#content">Zum Inhalt springen</a>
    <nav class="topbar shell" aria-label="Rechtliche Hinweise">
      <a class="wordmark" href="../"><img src="../icon.png" alt="" /><span>ToskLight</span></a>
      <div class="nav-links">
        <a href="../${IMPRESSUM_PATH}">Impressum</a>
        <a href="../${DATENSCHUTZ_PATH}">Datenschutz</a>
        <a class="nav-cta" href="../">Zur Startseite</a>
      </div>
    </nav>
    <main class="legal-shell shell" id="content">
      <header class="legal-hero">
        <p class="eyebrow">Rechtliche Hinweise</p>
        <h1>${heading}</h1>
        <p>${lede}</p>
      </header>
      <div class="legal-layout">
        <aside class="legal-toc" aria-label="Inhalt">
          <strong>Auf dieser Seite</strong>
          ${tocLinks}
        </aside>
        <article class="legal-content">
${body}
        </article>
      </div>
    </main>
    <footer><div class="shell download-footer"><p>${legalLinks("../")} · <a href="../license/">ToskLight Community License</a></p><a href="../">← Zurück zu ToskLight</a></div></footer>
  </body>
</html>
`;
}

const section = (id, label, heading, content) =>
	`          <section${id ? ` id="${id}"` : ""}${id === "english" ? ' lang="en"' : ""}>
            ${label ? `<p class="legal-section-label">${label}</p>\n            ` : ""}<h2>${heading}</h2>
            ${content}
          </section>`;

export function renderImpressum(contact) {
	const body = [
		section(
			"anbieter",
			"Angaben gemäß § 5 DDG",
			"Impressum",
			`<address>
              <strong>${escapeHtml(contact.operator)}</strong><br />
              ${escapeHtml(contact.name)}<br />
              ${postalAddress(contact)}
            </address>`,
		),
		section(
			"kontakt",
			"",
			"Kontakt",
			`<dl class="legal-details">
              <div><dt>E-Mail</dt><dd>${emailLink(contact.email)}</dd></div>
              <div><dt>Telefon</dt><dd>${phoneLink(contact.phone)}</dd></div>
            </dl>`,
		),
		section(
			"verantwortlich",
			"§ 18 Abs. 2 MStV",
			"Verantwortlich für den Inhalt",
			`<address>
              ${escapeHtml(contact.responsibleForContent)}<br />
              ${postalAddress(contact)}
            </address>`,
		),
		section(
			"streitbeilegung",
			"§ 36 VSBG",
			"Verbraucherstreitbeilegung",
			`<p>Wir sind nicht bereit und nicht verpflichtet, an Streitbeilegungsverfahren vor einer Verbraucherschlichtungsstelle teilzunehmen.</p>`,
		),
		section(
			"datenschutz",
			"",
			"Datenschutz",
			`<p>Informationen zur Verarbeitung personenbezogener Daten auf dieser Webseite finden Sie in der <a href="../${DATENSCHUTZ_PATH}">Datenschutzerklärung</a>.</p>`,
		),
		section(
			"english",
			"English summary",
			"Legal notice",
			`<p>This website is provided by ${escapeHtml(contact.operator)}, ${escapeHtml(contact.name)}, ${escapeHtml(contact.street)}, ${escapeHtml(contact.postalCode)} ${escapeHtml(contact.city)}, ${escapeHtml(contact.country)}. Contact details and the person responsible for the content are listed above. We neither are obliged nor willing to take part in dispute resolution proceedings before a consumer arbitration board. This English text is a convenience translation; in case of any discrepancy, the German version prevails.</p>`,
		),
	].join("\n");
	return page({
		title: "Impressum",
		description: "Impressum der ToskLight-Webseite: Anbieterkennzeichnung nach § 5 DDG und § 18 MStV.",
		heading: "Impressum.",
		lede: "Anbieterkennzeichnung für die öffentliche ToskLight-Webseite mit Produktinformationen, Handbuch und Test-Downloads.",
		toc: [
			["anbieter", "Anbieter"],
			["kontakt", "Kontakt"],
			["verantwortlich", "Verantwortlich"],
			["streitbeilegung", "Streitbeilegung"],
			["english", "English"],
		],
		body,
	});
}

export function renderDatenschutz(contact) {
	const body = [
		section(
			"verantwortlicher",
			"Art. 13 Abs. 1 lit. a DSGVO",
			"Verantwortlicher",
			`<p>Verantwortlich für die Verarbeitung personenbezogener Daten auf dieser Webseite ist:</p>
            <address>
              <strong>${escapeHtml(contact.operator)}</strong><br />
              ${escapeHtml(contact.name)}<br />
              ${postalAddress(contact)}
            </address>
            <p>E-Mail: ${emailLink(contact.email)} · Telefon: ${phoneLink(contact.phone)}</p>`,
		),
		section(
			"ueberblick",
			"",
			"Überblick",
			`<p>Diese Webseite ist ein statisches Informationsangebot zu ToskLight: Produktseiten, Handbuch, Komponenten-Katalog (Storybook), Code-Rundgang, Lizenzen und Links zu den Test-Downloads. Es gibt kein Kontaktformular, keine Benutzerkonten, keine Kommentarfunktion, keine Werbung und keine Reichweitenmessung oder sonstige Analyse des Nutzungsverhaltens.</p>
            <p>Alle Schriften, Skripte, Stylesheets, Bilder und Videos werden vom selben Server ausgeliefert wie die Seite selbst. Es werden keine Inhalte von Drittanbietern eingebunden, etwa Web-Fonts, Content-Delivery-Networks, Social-Media-Plugins oder eingebettete Videos.</p>`,
		),
		section(
			"hosting",
			"",
			"Hosting durch GitHub Pages",
			`<p>Diese Webseite wird über GitHub Pages bereitgestellt, einen Dienst der GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA („GitHub“).</p>
            <p>Beim Aufruf jeder Seite verarbeitet GitHub technisch notwendige Server-Logdaten, insbesondere Ihre IP-Adresse, Datum und Uhrzeit des Abrufs, die angeforderte Datei, den HTTP-Statuscode, die übertragene Datenmenge, die Referrer-URL sowie Angaben zu Browser und Betriebssystem. GitHub verarbeitet diese Daten, um die Webseite auszuliefern und um die Sicherheit und Stabilität des Dienstes zu gewährleisten; dazu protokolliert GitHub die IP-Adressen der Besucher.</p>
            <p>Rechtsgrundlage ist Art. 6 Abs. 1 lit. f DSGVO. Unser berechtigtes Interesse liegt in der zuverlässigen, sicheren und missbrauchsgeschützten Bereitstellung dieses Informationsangebotes.</p>
            <p>Die Daten werden in die USA übermittelt. GitHub ist nach dem EU-US Data Privacy Framework zertifiziert; die Übermittlung stützt sich auf den Angemessenheitsbeschluss der Europäischen Kommission (Art. 45 DSGVO) sowie zusätzlich auf die Standardvertragsklauseln der Europäischen Kommission (Art. 46 Abs. 2 lit. c DSGVO).</p>
            <p>Auf die Speicherdauer der Server-Logdaten bei GitHub haben wir keinen Einfluss. Weitere Informationen finden Sie im <a href="${GITHUB_PRIVACY_STATEMENT}">GitHub General Privacy Statement</a>.</p>`,
		),
		section(
			"cookies",
			"",
			"Cookies, lokaler Speicher und Tracking",
			`<p>Diese Webseite setzt keine Cookies und verwendet keine Tracking- oder Analysewerkzeuge.</p>
            <p>Der Komponenten-Katalog (<code>/storybook/</code>) und der Code-Rundgang (<code>/safari/</code>) legen Bedienzustände wie zuletzt angesehene Einträge, Panel-Einstellungen und Anzeigeoptionen im lokalen Speicher Ihres Browsers (Local Storage bzw. Session Storage) ab. Diese Daten verbleiben auf Ihrem Gerät, werden nicht an uns oder Dritte übertragen und dienen ausschließlich dazu, die von Ihnen aufgerufene Funktion bereitzustellen (§ 25 Abs. 2 Nr. 2 TDDDG). Sie können sie jederzeit über die Einstellungen Ihres Browsers löschen.</p>`,
		),
		section(
			"links",
			"",
			"Downloads und externe Links",
			`<p>Downloads, Release-Hinweise und Quellcode werden über <code>github.com</code> bereitgestellt; weitere Links führen zu Webseiten anderer Anbieter. Erst wenn Sie einen solchen Link anklicken, stellt Ihr Browser eine Verbindung zu dem jeweiligen Anbieter her. Für die dortige Verarbeitung ist der jeweilige Anbieter verantwortlich.</p>`,
		),
		section(
			"kontakt",
			"",
			"Kontaktaufnahme per E-Mail oder Telefon",
			`<p>Wenn Sie uns per E-Mail oder Telefon kontaktieren, verarbeiten wir die von Ihnen mitgeteilten Daten (etwa Name, E-Mail-Adresse, Telefonnummer und den Inhalt Ihrer Anfrage) ausschließlich, um Ihre Anfrage zu bearbeiten und zu beantworten.</p>
            <p>Rechtsgrundlage ist Art. 6 Abs. 1 lit. b DSGVO, soweit Ihre Anfrage mit einem Vertrag oder vorvertraglichen Maßnahmen zusammenhängt, und im Übrigen Art. 6 Abs. 1 lit. f DSGVO; unser berechtigtes Interesse liegt in der Beantwortung von Anfragen. Wir geben diese Daten nicht ohne Ihre Einwilligung weiter und löschen sie, sobald die Anfrage abschließend bearbeitet ist und keine gesetzlichen Aufbewahrungspflichten entgegenstehen.</p>`,
		),
		section(
			"pflicht",
			"",
			"Bereitstellung der Daten",
			`<p>Sie sind weder gesetzlich noch vertraglich verpflichtet, personenbezogene Daten bereitzustellen. Ohne die Übermittlung Ihrer IP-Adresse kann die Webseite jedoch technisch nicht ausgeliefert werden.</p>`,
		),
		section(
			"rechte",
			"Art. 15–21 DSGVO",
			"Ihre Rechte",
			`<p>Sie haben nach Maßgabe der gesetzlichen Voraussetzungen das Recht auf Auskunft (Art. 15 DSGVO), Berichtigung (Art. 16 DSGVO), Löschung (Art. 17 DSGVO), Einschränkung der Verarbeitung (Art. 18 DSGVO) und Datenübertragbarkeit (Art. 20 DSGVO). Wenden Sie sich dazu an die oben genannten Kontaktdaten.</p>
            <p><strong>Widerspruchsrecht (Art. 21 DSGVO):</strong> Soweit wir Daten auf Grundlage von Art. 6 Abs. 1 lit. f DSGVO verarbeiten, können Sie aus Gründen, die sich aus Ihrer besonderen Situation ergeben, jederzeit Widerspruch gegen diese Verarbeitung einlegen.</p>`,
		),
		section(
			"beschwerde",
			"Art. 77 DSGVO",
			"Beschwerderecht",
			`<p>Sie haben das Recht, sich bei einer Datenschutz-Aufsichtsbehörde zu beschweren. Für uns zuständig ist:</p>
            <address>
              Der Hessische Beauftragte für Datenschutz und Informationsfreiheit<br />
              Postfach 3163<br />
              65021 Wiesbaden<br />
              <a href="${HESSEN_DPA}">datenschutz.hessen.de</a>
            </address>`,
		),
		section(
			"automatisiert",
			"",
			"Keine automatisierte Entscheidungsfindung",
			`<p>Eine automatisierte Entscheidungsfindung einschließlich Profiling im Sinne von Art. 22 DSGVO findet nicht statt.</p>`,
		),
		section(
			"stand",
			"",
			"Stand und Änderungen",
			`<p>Wir passen diese Datenschutzerklärung an, wenn sich die Webseite, die eingesetzten Dienste oder die Rechtslage ändern. Stand: ${LEGAL_TEXT_DATE}.</p>`,
		),
		section(
			"english",
			"English summary",
			"Privacy policy",
			`<p>The controller is ${escapeHtml(contact.operator)}, ${escapeHtml(contact.name)} (contact details above). This static website is hosted on GitHub Pages by GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA. GitHub processes server log data including your IP address to deliver the site and keep it secure (Art. 6(1)(f) GDPR). Data is transferred to the USA under the EU-US Data Privacy Framework, under which GitHub is certified, and Standard Contractual Clauses; see the <a href="${GITHUB_PRIVACY_STATEMENT}">GitHub General Privacy Statement</a>. The site sets no cookies, uses no tracking or analytics and loads no third-party content; Storybook and the code tour keep interface state in your browser's local storage only. Emails and calls are used only to answer your request. You have the rights under Art. 15–21 GDPR and may complain to a supervisory authority, for us Der Hessische Beauftragte für Datenschutz und Informationsfreiheit. There is no automated decision-making. This English text is a convenience translation; in case of any discrepancy, the German version prevails.</p>`,
		),
	].join("\n");
	return page({
		title: "Datenschutzerklärung",
		description: "Datenschutzerklärung der ToskLight-Webseite nach Art. 13 DSGVO.",
		heading: "Datenschutz&shy;erklärung.",
		lede: "Welche personenbezogenen Daten beim Besuch dieser Webseite verarbeitet werden, zu welchem Zweck und welche Rechte Sie haben.",
		toc: [
			["verantwortlicher", "Verantwortlicher"],
			["hosting", "GitHub Pages"],
			["cookies", "Cookies"],
			["kontakt", "Kontakt"],
			["rechte", "Ihre Rechte"],
			["beschwerde", "Beschwerde"],
			["english", "English"],
		],
		body,
	});
}

/** The old combined page's URL stays reachable and points visitors at the two new pages. */
export function renderImprintRedirect() {
	return `<!doctype html>
<html lang="de">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta http-equiv="refresh" content="0; url=../${IMPRESSUM_PATH}" />
    <title>Impressum — ToskLight</title>
    <link rel="canonical" href="../${IMPRESSUM_PATH}" />
    <link rel="stylesheet" href="../site.css" />
  </head>
  <body class="legal-page">
    <main class="legal-shell shell"><p>Diese Seite ist umgezogen: ${legalLinks("../")}</p></main>
  </body>
</html>
`;
}
