customElements.define(
  "uc-palette",
  class extends HTMLElement {
    connectedCallback() {
      this.innerHTML = `
            <sdpi-item label="Palette">
                <sdpi-select
                    setting="palette"
                    datasource="getPalettes"
                    default=""
                    hot-reload
                    loading="Loading palettes...">
                </sdpi-select>
            </sdpi-item>
        `;

      this.querySelector("sdpi-item").setAttribute(
        "label",
        this.getAttribute("label") || "Palette",
      );
    }
  },
);
