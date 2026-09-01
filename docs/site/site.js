const lightbox = document.getElementById("image-lightbox");
const lightboxImage = document.getElementById("lightbox-image");
const lightboxCaption = document.getElementById("lightbox-caption");

if (lightbox instanceof HTMLDialogElement && lightboxImage && lightboxCaption) {
	document.querySelectorAll(".shot-image").forEach((control) => {
		control.addEventListener("click", () => {
			const figure = control.closest(".shot");
			const thumbnail = figure?.querySelector("img");
			if (!figure || !thumbnail) return;
			lightboxImage.src = thumbnail.currentSrc || thumbnail.src;
			lightboxImage.alt = thumbnail.alt;
			lightboxCaption.textContent = figure.querySelector("figcaption")?.textContent || "";
			lightbox.showModal();
		});
	});
	lightbox.querySelector(".lightbox-close")?.addEventListener("click", () => lightbox.close());
	lightbox.addEventListener("click", (event) => {
		if (event.target === lightbox) lightbox.close();
	});
}

const platformSelect = document.getElementById("platform-select");
const platformDownload = document.getElementById("platform-download");
const platformNote = document.getElementById("platform-note");

if (
	platformSelect instanceof HTMLSelectElement &&
	platformDownload instanceof HTMLAnchorElement &&
	platformNote
) {
	const detectedPlatform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
	if (detectedPlatform.includes("mac")) platformSelect.value = "macos";
	else if (detectedPlatform.includes("win")) platformSelect.value = "windows";
	else if (detectedPlatform.includes("linux")) platformSelect.value = "linux";

	const updatePlatform = () => {
		const option = platformSelect.selectedOptions[0];
		if (!option) return;
		platformDownload.href = option.dataset.url || "#";
		platformDownload.textContent = `Download for ${option.dataset.label || option.textContent}`;
		platformNote.textContent = option.dataset.note || "";
	};
	platformSelect.addEventListener("change", updatePlatform);
	updatePlatform();
}

document.querySelectorAll(".topbar").forEach((topbar) => {
	const links = topbar.querySelector(".nav-links");
	if (!links) return;
	const applications = [...links.querySelectorAll("a")].find((link) =>
		link.getAttribute("href")?.endsWith("#applications"),
	);
	if (applications instanceof HTMLAnchorElement) {
		const picker = document.createElement("div");
		picker.className = "application-picker";
		const trigger = document.createElement("button");
		trigger.type = "button";
		trigger.className = "nav-application-toggle";
		trigger.textContent = "Applications";
		trigger.setAttribute("aria-expanded", "false");
		const menu = document.createElement("div");
		menu.className = "application-menu";
		const prefix = topbar.querySelector(".wordmark")?.getAttribute("href") === "../" ? "../" : "";
		menu.innerHTML = `<a href="${prefix}control/">ToskLight Control</a><a href="${prefix}pixel/">ToskLight Pixel</a><a href="${prefix}architect/">ToskLight Architect</a><a href="${prefix}#applications">All applications</a>`;
		trigger.addEventListener("click", () => {
			const open = picker.classList.toggle("is-open");
			trigger.setAttribute("aria-expanded", String(open));
		});
		picker.append(trigger, menu);
		applications.replaceWith(picker);
	}
	const burger = document.createElement("button");
	burger.type = "button";
	burger.className = "menu-toggle";
	burger.setAttribute("aria-expanded", "false");
	burger.setAttribute("aria-label", "Open navigation menu");
	burger.innerHTML = '<span></span><span></span><span></span>';
	burger.addEventListener("click", () => {
		const open = topbar.classList.toggle("menu-open");
		burger.setAttribute("aria-expanded", String(open));
		burger.setAttribute("aria-label", open ? "Close navigation menu" : "Open navigation menu");
	});
	topbar.append(burger);
});
