import logoUrl from "../../assets/aix-logo.png";

export function AixLogo({ size = 30 }: { size?: number }) {
  return (
    <img
      src={logoUrl}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      style={{ borderRadius: size * 0.19, display: "block" }}
    />
  );
}
