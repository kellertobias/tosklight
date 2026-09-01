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
