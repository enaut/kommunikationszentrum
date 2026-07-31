document.addEventListener("DOMContentLoaded", async () => {
    for (const img of document.querySelectorAll('img[src$=".svg"]')) {
        try {
            const svg = await fetch(img.src).then(r => r.text());

            const modified = svg.replace(/UNLICENSED COPY/g, "");

            const blob = new Blob([modified], {
                type: "image/svg+xml"
            });

            img.src = URL.createObjectURL(blob);
        } catch (e) {
            console.error(e);
        }
    }
});
