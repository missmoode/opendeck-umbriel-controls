customElements.define(
  "uc-global-settings",
  class extends HTMLElement {
    connectedCallback() {
      this.innerHTML = `
      <details class="uc-global-settings">
        <summary>Global settings</summary>
        <sdpi-item label="Default palette">
          <sdpi-select
            setting="palette"
            global
            default=""
            datasource="getGlobalPalettes"
            hot-reload
            loading="Loading palettes...">
          </sdpi-select>
        </sdpi-item>
        <p class="uc-settings-help">Used by buttons set to Global. Defaults can be overridden here.</p>
      </details>
    `;
    }
  },
);
