import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";

const inter = Inter({ subsets: ["latin"] });

export const metadata: Metadata = {
  title: "PipelineX Dashboard",
  description: "CI/CD Bottleneck Analyzer & Optimizer",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${inter.className} bg-slate-900 text-slate-100 antialiased`}
      >
        {children}
      </body>
    </html>
  );
}
