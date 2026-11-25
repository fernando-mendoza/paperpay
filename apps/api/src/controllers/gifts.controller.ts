import { Request, Response } from 'express';
import { STELLAR_NETWORK, X402_HEADERS, type GiftClaimRequest, type GiftClaimResponse, type GiftSentResponse } from '@paperpay/shared';
import { GiftError, giftService } from '../services/gift.service';
import { jwtService } from '../services/jwt.service';
import { papersService } from '../services/papers.service';
import { SplitterError, splitterService } from '../services/splitter.service';
import { x402Service } from '../services/x402.service';

export class GiftsController {
  /** `POST /api/papers/:id/gift`: envía el regalo firmado (PAYMENT-SIGNATURE). Devuelve el recibo, no el artículo. */
  public async sendGift(req: Request, res: Response): Promise<void> {
    const { id } = req.params;
    if (!papersService.getPaperById(id)) {
      res.status(404).json({ error: 'PAPER_NOT_FOUND', message: `El artículo con ID '${id}' no existe en el catálogo.` });
      return;
    }
    if (!splitterService.enabled) {
      res.status(404).json({ error: 'SPLITTER_DISABLED', message: 'Esta API no cobra a través del contrato de reparto.' });
      return;
    }
    const raw = req.headers[X402_HEADERS.PAYMENT_SIGNATURE] as string | undefined;
    try {
      if (!raw) throw new SplitterError(400, 'Falta la cabecera PAYMENT-SIGNATURE con el regalo firmado.');
      let payload;
      try {
        payload = x402Service.parseSignatureHeader(raw);
      } catch {
        throw new SplitterError(400, 'Formato de cabecera PAYMENT-SIGNATURE inválido.');
      }
      if (payload.scheme !== 'exact' || payload.network !== STELLAR_NETWORK) {
        throw new SplitterError(400, 'El regalo debe ser un pago exact en Stellar testnet.');
      }
      const sent = await giftService.send(payload, id);
      const body: GiftSentResponse = { txHash: sent.txHash, paperId: id, reader: sent.reader, settledAt: sent.settledAt };
      res.status(200).json(body);
    } catch (err: unknown) {
      if (!(err instanceof SplitterError)) console.error('[GiftsController] Error inesperado al enviar el regalo:', err);
      const error = err instanceof SplitterError ? err : new SplitterError(502, 'No fue posible enviar el regalo.');
      res.status(error.status).json({
        error: error.status === 504 ? 'PAYMENT_PENDING' : 'GIFT_FAILED',
        message: error.message,
        ...(error.txHash && { txHash: error.txHash }),
      });
    }
  }

  /** `POST /api/gifts/claim`: quien abre el enlace firma con su llave y recibe el acceso de 24 h. */
  public async claimGift(req: Request, res: Response): Promise<void> {
    try {
      const receipt = await giftService.claim((req.body ?? {}) as Partial<GiftClaimRequest>);
      const accessToken = jwtService.issueAccessToken({ sub: receipt.reader, paperId: receipt.paperId, txHash: receipt.txHash });
      const body: GiftClaimResponse = {
        paperId: receipt.paperId,
        paperTitle: papersService.getPaperById(receipt.paperId)?.title ?? null,
        accessToken,
        txHash: receipt.txHash,
        settledAt: receipt.settledAt,
      };
      res.status(200).json(body);
    } catch (err: unknown) {
      if (err instanceof GiftError) {
        res.status(err.status).json({ error: err.code, message: err.message });
        return;
      }
      console.error('[GiftsController] Error inesperado al reclamar el regalo:', err);
      res.status(502).json({ error: 'GIFT_CLAIM_FAILED', message: 'No fue posible abrir el regalo.' });
    }
  }
}

export const giftsController = new GiftsController();
