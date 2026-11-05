import { type CSSProperties } from "react";
import { Icon } from "./whats-new-icon";
import { useWhatsNewMessages } from "./whats-new-messages";
import type { WhatsNewPageId } from "./whats-new-pages";

// Kevin's drawings from Syka Legacy, with Whispera's content. Each keeps the
// original markup and classes so whats-new.css animates it unchanged.

type Translate = ReturnType<typeof useWhatsNewMessages>;

const WAVE_LEVELS = [
  0.32, 0.58, 0.9, 0.62, 0.4, 0.74, 1, 0.66, 0.46, 0.82, 0.55, 0.36, 0.7,
  0.94, 0.5, 0.3,
];
const SCHEMES = ["light", "dark", "system"] as const;
const SCHEME_LABELS = { light: "art.original", dark: "art.folder3d", system: "art.metallic" } as const;
const SWATCHES = ["plain", "grain", "duo"] as const;

function cssVars(vars: Record<string, string | number>): CSSProperties {
  return vars as CSSProperties;
}

function ArtIcon({ name }: { name: string }) {
  return <Icon name={name} className="syka-wn-i" aria-hidden />;
}

/** Dictado local: la caja de dictado de Kevin, con el motor de esta PC. */
function DictationArt({ t }: { t: Translate }) {
  // The composer writing itself (Kevin's typed text and caret), not the breathing glow.
  return (
    <div className="syka-wn-composer syka-wn-glass">
      <div className="syka-wn-input">
        <span className="syka-wn-typed">{t("art.dictated")}</span>
        <span className="syka-wn-caret" />
      </div>
      <div className="syka-wn-actions">
        <span className="syka-wn-wave">
          {WAVE_LEVELS.map((level, index) => (
            <span
              key={index}
              style={cssVars({ "--i": index, "--level": level })}
            />
          ))}
        </span>
        <span className="syka-wn-chip" data-quiet="">
          {t("art.engineChip")}
        </span>
        <span className="syka-wn-send">
          <span className="syka-wn-stop" />
        </span>
      </div>
    </div>
  );
}

/** Elegir motor: las filas de cuentas de Syka Code, una por motor; arriba, el que está en uso. */
function EnginesArt({ t }: { t: Translate }) {
  const rows = [
    { name: t("art.accountWork"), meta: t("art.accountWorkMeta"), top: true, icon: "Cpu" },
    { name: t("art.accountPersonal"), meta: t("art.accountPersonalMeta"), top: false, icon: "Cloud" },
    { name: t("art.accountEngine"), meta: t("art.accountEngineMeta"), top: false, icon: "Laptop" },
  ];
  return (
    <div className="syka-wn-accounts syka-wn-glass">
      {rows.map((row, index) => (
        <div key={row.name} className="syka-wn-account" data-top={row.top ? "" : undefined} style={cssVars({ "--i": index })}>
          <ArtIcon name="GripVertical" />
          <span className="syka-wn-account-avatar"><Icon name={row.icon} aria-hidden width={14} height={14} /></span>
          <span className="syka-wn-account-name">
            <b>{row.name}</b>
            <small>{row.meta}</small>
          </span>
          {row.top ? <span className="syka-wn-account-chip">{t("art.accountInUse")}</span> : null}
        </div>
      ))}
    </div>
  );
}

/** Detector de voz: la actividad de Kevin contando lo que pasa con un golpe al micrófono. */
function SilenceArt({ t }: { t: Translate }) {
  return (
    <div className="syka-wn-activity syka-wn-glass">
      <div className="syka-wn-row">
        <ArtIcon name="Mic" />
        <span className="syka-wn-shine">
          {t("art.listening")} <code>{t("art.bump")}</code>
        </span>
      </div>
      <div className="syka-wn-plan">
        <div className="syka-wn-row">
          <ArtIcon name="AudioLines" />
          <span>{t("art.detector")}</span>
          <span className="syka-wn-row-meta">{t("art.noVoice")}</span>
        </div>
        <span className="syka-wn-progress">
          <span />
        </span>
      </div>
      <div className="syka-wn-row" data-tone="error">
        <ArtIcon name="CircleX" />
        <span>
          {t("art.dropped")}: <code>“Gracias”</code>
        </span>
      </div>
      <div className="syka-wn-row" data-tone="quiet">
        <ArtIcon name="Check" />
        <span>{t("art.nothingSent")}</span>
      </div>
    </div>
  );
}

/** Capturas de Kazu: la tarjeta de marca de Kevin, con el logo de Whispera. */
function CaptureArt({ t }: { t: Translate }) {
  return (
    <div className="syka-wn-brand">
      <div className="syka-wn-mark">
        <img src="/cristal/64x64.png" alt="" width={56} height={56} className="whispera-wn-logo" />
        <span>Whispera</span>
      </div>
      <div className="syka-wn-catalog syka-wn-glass">
        <div className="syka-wn-catalog-head">
          <span>{t("art.captures")}</span>
          <span className="syka-wn-fused">
            <span>
              <ArtIcon name="ScanLine" />
              {t("art.newCapture")}
            </span>
            <i />
            <span>
              <ArtIcon name="ChevronDown" />
            </span>
          </span>
        </div>
        <div className="syka-wn-ext">
          <span className="syka-wn-ext-icon">
            <ArtIcon name="Frame" />
          </span>
          <span className="syka-wn-ext-text">
            <strong>{t("art.thinBorder")}</strong>
            <small>{t("art.thinBorderBody")}</small>
          </span>
          <span className="syka-wn-switch" />
        </div>
        <div className="syka-wn-ext">
          <span className="syka-wn-ext-icon">
            <ArtIcon name="Video" />
          </span>
          <span className="syka-wn-ext-text">
            <strong>{t("art.videoLikeImage")}</strong>
            <small>{t("art.videoLikeImageBody")}</small>
          </span>
          <span className="syka-wn-switch" />
        </div>
      </div>
    </div>
  );
}

/** Carpeta 3D: la apariencia de Kevin, con los dibujos de la carpeta. */
function FolderArt({ t }: { t: Translate }) {
  return (
    <div className="syka-wn-look">
      <div className="syka-wn-tiles">
        {SCHEMES.map((scheme) => (
          <span key={scheme} className="syka-wn-tile" data-scheme={scheme}>
            <span className="syka-wn-window">
              <span className="syka-wn-window-side" />
              <span className="syka-wn-window-main">
                <span className="syka-wn-window-bar" />
              </span>
            </span>
            <span className="syka-wn-tile-label">{t(SCHEME_LABELS[scheme])}</span>
          </span>
        ))}
      </div>
      <div className="syka-wn-panel syka-wn-glass">
        <div className="syka-wn-row">
          <span>{t("art.folderColour")}</span>
          <span className="syka-wn-swatches">
            {SWATCHES.map((swatch) => (
              <span
                key={swatch}
                data-swatch={swatch}
                data-active={swatch === "grain" ? "" : undefined}
              />
            ))}
          </span>
        </div>
        <div className="syka-wn-row">
          <span>{t("art.scale")}</span>
          <span className="syka-wn-slider">
            <span />
          </span>
        </div>
        <div className="syka-wn-row">
          <span>{t("art.animations")}</span>
          <span className="syka-wn-switch" />
        </div>
      </div>
    </div>
  );
}

export function WhatsNewArt({ page }: { page: WhatsNewPageId }) {
  const t = useWhatsNewMessages();
  switch (page) {
    case "local":
      return <DictationArt t={t} />;
    case "setup":
      return <EnginesArt t={t} />;
    case "silence":
      return <SilenceArt t={t} />;
    case "capture":
      return <CaptureArt t={t} />;
    case "folder":
      return <FolderArt t={t} />;
  }
}
